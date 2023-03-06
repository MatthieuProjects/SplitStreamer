use serde::{Serialize, Deserialize};

// JSON messages we communicate with
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PeerPacketInner {
    #[serde(rename = "ice")]
    Ice {
        candidate: String,
        #[serde(rename = "sdpMLineIndex")]
        sdp_mline_index: u32,
    },
    #[serde(rename = "sdp")]
    Sdp {
        #[serde(rename = "type")]
        type_: String,
        sdp: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct PeerPacket {
    pub peer: String,
    #[serde(flatten)]
    pub inner: PeerPacketInner,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Message {
    #[serde(rename = "hello")]
    Hello {
        id: String,
    },
    #[serde(rename = "join")]
    Join(),
    #[serde(rename = "client_message")]
    ClientMessage(PeerPacket),
    #[serde(rename = "server_message")]
    ServerMessage(PeerPacket),
    #[serde(rename = "client_disconnect")]
    ClientDisconnect {
        peer: String,
    },
    #[serde(rename = "join_ack")]
    JoinACK {},
    #[serde(rename = "client_join")]
    ClientJoin {
        peer: String,
    },
}
