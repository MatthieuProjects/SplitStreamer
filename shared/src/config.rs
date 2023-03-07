use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VideoBox {
    pub top: usize,
    pub bottom: usize,
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ClientConfig {
    pub multicast_port: u16,
    pub multicast_address: String,
    pub video_box: VideoBox,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ClientConfigFile {
    pub configs: Vec<ClientConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct Resolution {
    pub height: u64,
    pub width: u64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct ServerConfig {
    pub multicast_port: u16,
    pub multicast_address: String,
    pub total_resolution: Resolution,

    pub signaling_server: String,
    pub stun_server: String,
    pub turn_server: String,
}
