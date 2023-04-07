use shared::config::ServerConfig;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::mpsc;

use async_tungstenite::tungstenite;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::Stream;
use tungstenite::Message as WsMessage;

use gstreamer::{prelude::*};
use gstreamer::{glib, Element};

use anyhow::{anyhow, bail};

use crate::webrtc::peer::PeerInner;

use self::payloads::{Message, PeerPacketInner};
use self::peer::Peer;

pub mod payloads;
mod peer;

// upgrade weak reference or return
#[macro_export]
macro_rules! upgrade_weak {
    ($x:ident, $r:expr) => {{
        match $x.upgrade() {
            Some(o) => o,
            None => return $r,
        }
    }};
    ($x:ident) => {
        upgrade_weak!($x, ())
    };
}

// Strong reference to our application state
#[derive(Debug, Clone)]
pub struct App(Arc<AppInner>);

// Weak reference to our application state
#[derive(Debug, Clone)]
struct AppWeak(Weak<AppInner>);

// Actual application state
#[derive(Debug)]
pub struct AppInner {
    pipeline: gstreamer::Pipeline,
    audio_switch: Element,
    video_switch: Element,
    send_msg_tx: Arc<Mutex<mpsc::UnboundedSender<WsMessage>>>,
    peers: Mutex<BTreeMap<String, Peer>>,

    settings: ServerConfig,
}

// To be able to access the App's fields directly
impl<'a> std::ops::Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &AppInner {
        &self.0
    }
}

impl<'a> AppWeak {
    // Try upgrading a weak reference to a strong one
    fn upgrade(&'a self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

impl<'a> App {
    // Downgrade the strong reference to a weak reference
    fn downgrade(&'a self) -> AppWeak {
        AppWeak(Arc::downgrade(&self.0))
    }

    pub fn new(
        settings: ServerConfig,
    ) -> Result<
        (
            App,
            impl Stream<Item = gstreamer::Message>,
            impl Stream<Item = WsMessage>,
        ),
        anyhow::Error,
    > {
        // Create the GStreamer pipeline
        let pipeline = gstreamer::parse_launch(
            "videotestsrc ! fallbackswitch min-upstream-latency=5000 name=video_switch ! capsfilter caps=video/x-raw,width=1280,height=800,pixel-aspect-ratio=1/1 ! autovideosink audiotestsrc ! fallbackswitch name=audio_switch ! autoaudiosink",
        )?;

        // Downcast from gstreamer::Element to gstreamer::Pipeline
        let pipeline = pipeline
            .downcast::<gstreamer::Pipeline>()
            .expect("not a pipeline");

        // Get access to the tees and mixers by name
        let audio_switch = pipeline
            .by_name("audio_switch")
            .expect("Audio switch couldn't be found.");
        let video_switch = pipeline
            .by_name("video_switch")
            .expect("Video switch couldn't be found.");

        let default_sink = video_switch.iterate_sink_pads().next().unwrap().unwrap();
        default_sink.set_property("priority", 10u32);
        let default_sink = audio_switch.iterate_sink_pads().next().unwrap().unwrap();
        default_sink.set_property("priority", 10u32);
        
        // Create a stream for handling the GStreamer message asynchronously
        let bus = pipeline.bus().unwrap();
        let send_gst_msg_rx = bus.stream();

        // Channel for outgoing WebSocket messages from other threads
        let (send_ws_msg_tx, send_ws_msg_rx) = mpsc::unbounded_channel::<WsMessage>();

        let app = App(Arc::new(AppInner {
            pipeline,
            audio_switch,
            video_switch,
            peers: Mutex::new(BTreeMap::new()),
            send_msg_tx: Arc::new(Mutex::new(send_ws_msg_tx)),
            settings,
        }));

        // Asynchronously set the pipeline to Playing
        app.pipeline.call_async(|pipeline| {
            // If this fails, post an error on the bus so we exit
            if pipeline.set_state(gstreamer::State::Playing).is_err() {
                gstreamer::element_error!(
                    pipeline,
                    gstreamer::LibraryError::Failed,
                    ("Failed to set pipeline to Playing")
                );
            }
        });

        Ok((
            app,
            send_gst_msg_rx,
            UnboundedReceiverStream::new(send_ws_msg_rx),
        ))
    }

    // Handle WebSocket messages, both our own as well as WebSocket protocol messages
    pub fn handle_websocket_message(&self, msg: &str) -> Result<(), anyhow::Error> {
        let message: Message = serde_json::from_str(msg)?;

        match message {
            Message::Hello { id } => {
                println!("Joined as {id}");
                Ok(())
            }
            Message::ClientMessage(data) => {
                let peers = self.peers.lock().unwrap();
                let peer = peers
                    .get(&data.peer)
                    .ok_or_else(|| anyhow!("Can't find peer {}", data.peer))?
                    .clone();
                drop(peers);

                match data.inner {
                    PeerPacketInner::Sdp { type_, sdp } => peer.handle_sdp(&type_, &sdp),
                    PeerPacketInner::Ice {
                        sdp_mline_index,
                        candidate,
                    } => peer.handle_ice(sdp_mline_index, &candidate),
                }
            }
            Message::ClientDisconnect { peer } => self.remove_peer(&peer),
            Message::ClientJoin { peer } => self.add_peer(&peer, false),

            _ => unreachable!(),
        }
    }

    // Handle GStreamer messages coming from the pipeline
    pub fn handle_pipeline_message(
        &self,
        message: &gstreamer::Message,
    ) -> Result<(), anyhow::Error> {
        use gstreamer::message::MessageView;

        match message.view() {
            MessageView::Error(err) => bail!(
                "Error from element {}: {} ({})",
                err.src()
                    .map(|s| String::from(s.path_string()))
                    .unwrap_or_else(|| String::from("None")),
                err.error(),
                err.debug().unwrap_or_else(|| glib::GString::from("None")),
            ),
            MessageView::Warning(warning) => {
                println!("Warning: \"{}\"", warning.debug().unwrap());
            }
            MessageView::Latency(_) => {
                let _ = self.pipeline.recalculate_latency();
            }
            _ => (),
        }

        Ok(())
    }

    // Add this new peer and if requested, send the offer to it
    fn add_peer(&self, peer_id: &str, offer: bool) -> Result<(), anyhow::Error> {
        let mut peers = self.peers.lock().unwrap();
        if peers.contains_key(peer_id) {
            bail!("Peer {peer_id} already called");
        }

        let peer_bin = gstreamer::parse_bin_from_description("webrtcbin name=webrtcbin", false)?;

        // Get access to the webrtcbin by name
        let webrtcbin = peer_bin.by_name("webrtcbin").expect("can't find webrtcbin");

        // Set some properties on webrtcbin
        webrtcbin.set_property_from_str("stun-server", &self.settings.stun_server);
        webrtcbin.set_property_from_str("turn-server", &self.settings.turn_server);

        let peer = Peer(Arc::new(PeerInner {
            peer_id: peer_id.to_string(),
            bin: peer_bin,
            webrtcbin,
            send_msg_tx: self.send_msg_tx.clone(),
            settings: self.settings.clone(),
        }));

        // Insert the peer into our map_
        peers.insert(peer_id.to_string(), peer.clone());
        drop(peers);

        // Add to the whole pipeline
        self.pipeline.add(&peer.bin).unwrap();

        // If we should send the offer to the peer, do so from on-negotiation-needed
        if offer {
            // Connect to on-negotiation-needed to handle sending an Offer
            let peer_clone = peer.downgrade();
            peer.webrtcbin.connect_closure(
                "on-negotiation-needed",
                false,
                glib::closure!(move |_webrtcbin: &gstreamer::Element| {
                    let peer = upgrade_weak!(peer_clone);
                    if let Err(err) = peer.on_negotiation_needed() {
                        gstreamer::element_error!(
                            peer.bin,
                            gstreamer::LibraryError::Failed,
                            ("Failed to negotiate: {:?}", err)
                        );
                    }
                }),
            );
        }

        // Whenever there is a new ICE candidate, send it to the peer
        let peer_clone = peer.downgrade();
        peer.webrtcbin.connect_closure(
            "on-ice-candidate",
            false,
            glib::closure!(move |_webrtcbin: &gstreamer::Element,
                                 mlineindex: u32,
                                 candidate: &str| {
                let peer = upgrade_weak!(peer_clone);

                if let Err(err) = peer.on_ice_candidate(mlineindex, candidate) {
                    gstreamer::element_error!(
                        peer.bin,
                        gstreamer::LibraryError::Failed,
                        ("Failed to send ICE candidate: {:?}", err)
                    );
                }
            }),
        );

        // Whenever there is a new stream incoming from the peer, handle it
        let peer_clone = peer.downgrade();
        peer.webrtcbin.connect_pad_added(move |_webrtc, pad| {
            let peer = upgrade_weak!(peer_clone);

            if let Err(err) = peer.on_incoming_stream(pad) {
                gstreamer::element_error!(
                    peer.bin,
                    gstreamer::LibraryError::Failed,
                    ("Failed to handle incoming stream: {:?}", err)
                );
            }
        });

        // Whenever a decoded stream comes available, handle it and connect it to the mixers
        let app_clone = self.downgrade();
        peer.bin.connect_pad_added(move |_bin, pad| {
            let app = upgrade_weak!(app_clone);

            match pad.name().as_str() {
                "audio_src" => {
                    let sink_pad = app.audio_switch.request_pad_simple("sink_%u").unwrap();
                    pad.link(&sink_pad).unwrap();

                    sink_pad.connect_unlinked(move |pad, _peer| {
                        if let Some(audiomixer) = pad.parent() {
                            let audiomixer =
                                audiomixer.downcast_ref::<gstreamer::Element>().unwrap();
                            audiomixer.release_request_pad(pad);
                        }
                    });
                }
                "video_src" => {
                    let sink_pad = app.video_switch.request_pad_simple("sink_%u").unwrap();
                    pad.link(&sink_pad).unwrap();

                    sink_pad.connect_unlinked(move |pad, _peer| {
                        if let Some(videomixer) = pad.parent() {
                            let videomixer =
                                videomixer.downcast_ref::<gstreamer::Element>().unwrap();
                            videomixer.release_request_pad(pad);
                        }
                    });
                }
                &_ => {
                    unreachable!()
                }
            }
        });

        // Asynchronously set the peer bin to Playing
        peer.bin.call_async(move |bin| {
            // If this fails, post an error on the bus so we exit
            if bin.sync_state_with_parent().is_err() {
                gstreamer::element_error!(
                    bin,
                    gstreamer::LibraryError::Failed,
                    ("Failed to set peer bin to Playing")
                );
            }
        });

        Ok(())
    }

    // Remove this peer
    fn remove_peer(&self, peer_id: &str) -> Result<(), anyhow::Error> {
        println!("Removing peer {peer_id}");
        let mut peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.remove(peer_id) {
            drop(peers);

            // Now asynchronously remove the peer from the pipeline
            let app_clone = self.downgrade();
            self.pipeline.call_async(move |_pipeline| {
                let app = upgrade_weak!(app_clone);
                // Then remove the peer bin gracefully from the pipeline
                let _ = app.pipeline.remove(&peer.bin);
                let _ = peer.bin.set_state(gstreamer::State::Null);
                println!("Removed peer {}", peer.peer_id);
            });
        }

        Ok(())
    }
}

// Make sure to shut down the pipeline when it goes out of scope
// to release any system resources
impl Drop for AppInner {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gstreamer::State::Null);
    }
}
