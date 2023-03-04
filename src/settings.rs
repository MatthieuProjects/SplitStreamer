use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct VideoBox {
    pub top: usize,
    pub bottom: usize,
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConfig {
    pub multicast_port: u16,
    pub multicast_address: String,
    pub video_box: VideoBox,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConfigFile {
    pub configs: Vec<ClientConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub multicast_port: u16,
    pub multicast_address: String,

    pub resolution_w: i32,
    pub resolution_h: i32,
}
