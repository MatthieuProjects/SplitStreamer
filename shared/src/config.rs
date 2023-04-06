use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct ServerConfig {
    pub signaling_server: String,
    pub stun_server: String,
    pub turn_server: String,
}
