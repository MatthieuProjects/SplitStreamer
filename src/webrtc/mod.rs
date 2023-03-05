use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use async_std::prelude::*;
use futures::channel::mpsc;

use async_tungstenite::tungstenite;
use tungstenite::Message as WsMessage;

use gstreamer::{glib, Element};
use gstreamer::{prelude::*, ElementFactory};

use anyhow::{anyhow, bail};

use crate::webrtc::peer::PeerInner;

use self::payloads::{Message, PeerPacketInner};
use self::peer::Peer;

pub mod payloads;
mod peer;

const STUN_SERVER: &str = "stun://stun.l.google.com:19302";
const TURN_SERVER: &str = "turn://foo:bar@webrtc.gstreamer.net:3478";
pub const VIDEO_WIDTH: u32 = 1920;
pub const VIDEO_HEIGHT: u32 = 1080;

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
    fallback_switch: Element,
    send_msg_tx: Arc<Mutex<mpsc::UnboundedSender<WsMessage>>>,
    peers: Mutex<BTreeMap<String, Peer>>,
}

// To be able to access the App's fields directly
impl std::ops::Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &AppInner {
        &self.0
    }
}

impl AppWeak {
    // Try upgrading a weak reference to a strong one
    fn upgrade(&self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

impl App {
    // Downgrade the strong reference to a weak reference
    fn downgrade(&self) -> AppWeak {
        AppWeak(Arc::downgrade(&self.0))
    }

    pub fn new() -> Result<
        (
            Self,
            impl Stream<Item = gstreamer::Message>,
            impl Stream<Item = WsMessage>,
        ),
        anyhow::Error,
    > {
        // Create the GStreamer pipeline
        let pipeline = gstreamer::parse_launch(
            "fallbackswitch min-upstream-latency=5000 name=switch ! queue name=output ! autovideosink",
        )?;

        let test = ElementFactory::make("videotestsrc").build()?;

        // Downcast from gstreamer::Element to gstreamer::Pipeline
        let pipeline = pipeline
            .downcast::<gstreamer::Pipeline>()
            .expect("not a pipeline");
        pipeline.add(&test).unwrap();
        // Get access to the tees and mixers by name
        let fallback_switch = pipeline.by_name("switch").expect("can't find switch");
        let pad = fallback_switch.request_pad_simple("sink_%u").unwrap();
        pad.set_property_from_str("priority", "10");
        test.static_pad("src").unwrap().link(&pad).unwrap();

        let queue_output = pipeline.by_name("output").expect("can't find output");

        // Create a stream for handling the GStreamer message asynchronously
        let bus = pipeline.bus().unwrap();
        let send_gst_msg_rx = bus.stream();

        // Channel for outgoing WebSocket messages from other threads
        let (send_ws_msg_tx, send_ws_msg_rx) = mpsc::unbounded::<WsMessage>();

        let app = App(Arc::new(AppInner {
            pipeline,
            fallback_switch,
            peers: Mutex::new(BTreeMap::new()),
            send_msg_tx: Arc::new(Mutex::new(send_ws_msg_tx)),
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

        Ok((app, send_gst_msg_rx, send_ws_msg_rx))
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
        webrtcbin.set_property_from_str("stun-server", STUN_SERVER);
        webrtcbin.set_property_from_str("turn-server", TURN_SERVER);
        webrtcbin.set_property_from_str("bundle-policy", "max-bundle");

        let peer = Peer(Arc::new(PeerInner {
            peer_id: peer_id.to_string(),
            bin: peer_bin,
            webrtcbin,
            send_msg_tx: self.send_msg_tx.clone(),
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

            if pad.name() == "audio_src" {
                let audiomixer_sink_pad = ElementFactory::make("fakesink").build().unwrap();
                app.pipeline.add(&audiomixer_sink_pad).unwrap();

                let sink_pad = audiomixer_sink_pad.static_pad("sink").unwrap();
                pad.link(&sink_pad).unwrap();

                // Once it is unlinked again later when the peer is being removed,
                // also release the pad on the mixer
                sink_pad.connect_unlinked(move |pad, _peer| {
                    if let Some(audiomixer) = pad.parent() {
                        let audiomixer = audiomixer.downcast_ref::<gstreamer::Element>().unwrap();
                        audiomixer.release_request_pad(pad);
                    }
                });
                println!("audio is unhandled for now.");
            } else if pad.name() == "video_src" {
                let videomixer_sink_pad =
                    app.fallback_switch.request_pad_simple("sink_%u").unwrap();
                pad.link(&videomixer_sink_pad).unwrap();

                app.fallback_switch
                    .set_property("active-pad", &videomixer_sink_pad);
                // Once it is unlinked again later when the peer is being removed,
                // also release the pad on the mixer
                videomixer_sink_pad.connect_unlinked(move |pad, _peer| {
                    if let Some(videomixer) = pad.parent() {
                        let videomixer = videomixer.downcast_ref::<gstreamer::Element>().unwrap();
                        videomixer.release_request_pad(pad);
                    }
                });
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
