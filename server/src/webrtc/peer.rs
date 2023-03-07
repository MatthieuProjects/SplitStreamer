use std::sync::{Arc, Mutex, Weak};

use shared::config::ServerConfig;
use tokio::sync::mpsc;

use async_tungstenite::tungstenite;
use tungstenite::Message as WsMessage;

use gstreamer::prelude::*;

use anyhow::{anyhow, bail, Context};

use crate::upgrade_weak;
use crate::webrtc::payloads::{Message, PeerPacket, PeerPacketInner};

// Strong reference to the state of one peer
#[derive(Debug, Clone)]
pub struct Peer(pub Arc<PeerInner>);

// To be able to access the Peers's fields directly
impl std::ops::Deref for Peer {
    type Target = PeerInner;

    fn deref(&self) -> &PeerInner {
        &self.0
    }
}

// Weak reference to the state of one peer
#[derive(Debug, Clone)]
pub struct PeerWeak(Weak<PeerInner>);

impl PeerWeak {
    // Try upgrading a weak reference to a strong one
    pub fn upgrade(&self) -> Option<Peer> {
        self.0.upgrade().map(Peer)
    }
}

// Actual peer state
#[derive(Debug)]
pub struct PeerInner {
    pub peer_id: String,
    pub bin: gstreamer::Bin,
    pub webrtcbin: gstreamer::Element,
    pub send_msg_tx: Arc<Mutex<mpsc::UnboundedSender<WsMessage>>>,
    pub settings: ServerConfig,
}

impl Peer {
    // Downgrade the strong reference to a weak reference
    pub fn downgrade(&self) -> PeerWeak {
        PeerWeak(Arc::downgrade(&self.0))
    }

    // Whenever webrtcbin tells us that (re-)negotiation is needed, simply ask
    // for a new offer SDP from webrtcbin without any customization and then
    // asynchronously send it to the peer via the WebSocket connection
    pub fn on_negotiation_needed(&self) -> Result<(), anyhow::Error> {
        println!("starting negotiation with peer {}", self.peer_id);

        let peer_clone = self.downgrade();
        let promise = gstreamer::Promise::with_change_func(move |reply| {
            let peer = upgrade_weak!(peer_clone);

            if let Err(err) = peer.on_offer_created(reply) {
                gstreamer::element_error!(
                    peer.bin,
                    gstreamer::LibraryError::Failed,
                    ("Failed to send SDP offer: {:?}", err)
                );
            }
        });

        self.webrtcbin
            .emit_by_name::<()>("create-offer", &[&None::<gstreamer::Structure>, &promise]);

        Ok(())
    }

    // Once webrtcbin has create the offer SDP for us, handle it by sending it to the peer via the
    // WebSocket connection
    fn on_offer_created(
        &self,
        reply: Result<Option<&gstreamer::StructureRef>, gstreamer::PromiseError>,
    ) -> Result<(), anyhow::Error> {
        let reply = match reply {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                bail!("Offer creation future got no reponse");
            }
            Err(err) => {
                bail!("Offer creation future got error reponse: {err:?}");
            }
        };

        let offer = reply
            .value("offer")
            .unwrap()
            .get::<gstreamer_webrtc::WebRTCSessionDescription>()
            .expect("Invalid argument");
        self.webrtcbin.emit_by_name::<()>(
            "set-local-description",
            &[&offer, &None::<gstreamer::Promise>],
        );

        println!(
            "sending SDP offer to peer: {}",
            offer.sdp().as_text().unwrap()
        );

        let message = serde_json::to_string(&Message::ServerMessage(PeerPacket {
            peer: self.peer_id.clone(),
            inner: PeerPacketInner::Sdp {
                type_: "offer".to_string(),
                sdp: offer.sdp().as_text().unwrap(),
            },
        }))
        .unwrap();

        self.send_msg_tx
            .lock()
            .unwrap()
            .send(WsMessage::Text(message))
            .context("Failed to send SDP offer")?;

        Ok(())
    }

    // Once webrtcbin has create the answer SDP for us, handle it by sending it to the peer via the
    // WebSocket connection
    fn on_answer_created(
        &self,
        reply: Result<Option<&gstreamer::StructureRef>, gstreamer::PromiseError>,
    ) -> Result<(), anyhow::Error> {
        let reply = match reply {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                bail!("Answer creation future got no reponse");
            }
            Err(err) => {
                bail!("Answer creation future got error reponse: {err:?}");
            }
        };

        let answer = reply
            .value("answer")
            .unwrap()
            .get::<gstreamer_webrtc::WebRTCSessionDescription>()
            .expect("Invalid argument");
        self.webrtcbin.emit_by_name::<()>(
            "set-local-description",
            &[&answer, &None::<gstreamer::Promise>],
        );

        println!(
            "sending SDP answer to peer: {}",
            answer.sdp().as_text().unwrap()
        );

        let message = serde_json::to_string(&Message::ServerMessage(PeerPacket {
            peer: self.peer_id.clone(),
            inner: PeerPacketInner::Sdp {
                type_: "answer".to_string(),
                sdp: answer.sdp().as_text().unwrap(),
            },
        }))
        .unwrap();

        self.send_msg_tx
            .lock()
            .unwrap()
            .send(WsMessage::Text(message))
            .context("Failed to send SDP answer")?;

        Ok(())
    }

    // Handle incoming SDP answers from the peer
    pub fn handle_sdp(&self, type_: &str, sdp: &str) -> Result<(), anyhow::Error> {
        if type_ == "answer" {
            print!("Received answer:\n{sdp}\n");

            let ret = gstreamer_sdp::SDPMessage::parse_buffer(sdp.as_bytes())
                .map_err(|_| anyhow!("Failed to parse SDP answer"))?;
            let answer = gstreamer_webrtc::WebRTCSessionDescription::new(
                gstreamer_webrtc::WebRTCSDPType::Answer,
                ret,
            );

            self.webrtcbin.emit_by_name::<()>(
                "set-remote-description",
                &[&answer, &None::<gstreamer::Promise>],
            );

            Ok(())
        } else if type_ == "offer" {
            print!("Received offer:\n{sdp}\n");

            let ret = gstreamer_sdp::SDPMessage::parse_buffer(sdp.as_bytes())
                .map_err(|_| anyhow!("Failed to parse SDP offer"))?;

            // And then asynchronously start our pipeline and do the next steps. The
            // pipeline needs to be started before we can create an answer
            let peer_clone = self.downgrade();
            self.bin.call_async(move |_pipeline| {
                let peer = upgrade_weak!(peer_clone);

                let offer = gstreamer_webrtc::WebRTCSessionDescription::new(
                    gstreamer_webrtc::WebRTCSDPType::Offer,
                    ret,
                );

                peer.0.webrtcbin.emit_by_name::<()>(
                    "set-remote-description",
                    &[&offer, &None::<gstreamer::Promise>],
                );

                let peer_clone = peer.downgrade();
                let promise = gstreamer::Promise::with_change_func(move |reply| {
                    let peer = upgrade_weak!(peer_clone);

                    if let Err(err) = peer.on_answer_created(reply) {
                        gstreamer::element_error!(
                            peer.bin,
                            gstreamer::LibraryError::Failed,
                            ("Failed to send SDP answer: {:?}", err)
                        );
                    }
                });

                peer.0.webrtcbin.emit_by_name::<()>(
                    "create-answer",
                    &[&None::<gstreamer::Structure>, &promise],
                );
            });

            Ok(())
        } else {
            bail!("Sdp type is not \"answer\" but \"{type_}\"")
        }
    }

    // Handle incoming ICE candidates from the peer by passing them to webrtcbin
    pub fn handle_ice(&self, sdp_mline_index: u32, candidate: &str) -> Result<(), anyhow::Error> {
        self.webrtcbin
            .emit_by_name::<()>("add-ice-candidate", &[&sdp_mline_index, &candidate]);

        Ok(())
    }

    // Asynchronously send ICE candidates to the peer via the WebSocket connection as a JSON
    // message
    pub fn on_ice_candidate(&self, mlineindex: u32, candidate: &str) -> Result<(), anyhow::Error> {
        let message = serde_json::to_string(&Message::ServerMessage(PeerPacket {
            peer: self.peer_id.clone(),
            inner: PeerPacketInner::Ice {
                candidate: candidate.to_string(),
                sdp_mline_index: mlineindex,
            },
        }))
        .unwrap();
        self.send_msg_tx
            .lock()
            .unwrap()
            .send(WsMessage::Text(message))
            .context("Failed to send ICE candidate")?;

        Ok(())
    }

    // Whenever there's a new incoming, encoded stream from the peer create a new decodebin
    // and audio/video sink depending on the stream type
    pub fn on_incoming_stream(&self, pad: &gstreamer::Pad) -> Result<(), anyhow::Error> {
        // Early return for the source pads we're adding ourselves
        if pad.direction() != gstreamer::PadDirection::Src {
            return Ok(());
        }

        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();
        let media_type = s
            .get_optional::<&str>("media")
            .expect("Invalid type")
            .ok_or_else(|| anyhow!("no media type in caps {caps:?}"))?;

        let conv = if media_type == "video" {
            gstreamer::parse_bin_from_description(
                &format!(
                    "decodebin name=dbin ! queue ! videoconvert ! videoscale ! capsfilter name=src caps=video/x-raw,width={},height={},pixel-aspect-ratio=1/1", self.settings.total_resolution.height, self.settings.total_resolution.height
                ),
                false,
            )?
        } else if media_type == "audio" {
            gstreamer::parse_bin_from_description(
                "decodebin name=dbin ! queue ! audioconvert ! audioresample name=src",
                false,
            )?
        } else {
            println!("Unknown pad {pad:?}, ignoring");
            return Ok(());
        };

        // Add a ghost pad on our conv bin that proxies the sink pad of the decodebin
        let dbin = conv.by_name("dbin").unwrap();
        let sinkpad =
            gstreamer::GhostPad::with_target(Some("sink"), &dbin.static_pad("sink").unwrap())
                .unwrap();
        conv.add_pad(&sinkpad).unwrap();

        // And another one that proxies the source pad of the last element
        let src = conv.by_name("src").unwrap();
        let srcpad =
            gstreamer::GhostPad::with_target(Some("src"), &src.static_pad("src").unwrap()).unwrap();
        conv.add_pad(&srcpad).unwrap();

        self.bin.add(&conv).unwrap();
        conv.sync_state_with_parent()
            .with_context(|| format!("can't start sink for stream {caps:?}"))?;

        pad.link(&sinkpad)
            .with_context(|| format!("can't link sink for stream {caps:?}"))?;

        // And then add a new ghost pad to the peer bin that proxies the source pad we added above
        if media_type == "video" {
            let srcpad = gstreamer::GhostPad::with_target(Some("video_src"), &srcpad).unwrap();
            srcpad.set_active(true).unwrap();
            self.bin.add_pad(&srcpad).unwrap();
        } else if media_type == "audio" {
            let srcpad = gstreamer::GhostPad::with_target(Some("audio_src"), &srcpad).unwrap();
            srcpad.set_active(true).unwrap();
            self.bin.add_pad(&srcpad).unwrap();
        }

        Ok(())
    }
}

// At least shut down the bin here if it didn't happen so far
impl Drop for PeerInner{
    fn drop(&mut self) {
        let _ = self.bin.set_state(gstreamer::State::Null);
    }
}
