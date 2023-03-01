use serde::{Serialize, Deserialize};
/// 
#[derive(Debug, Deserialize, Serialize)]
pub struct ScreenConfig {
    pub screen_width: i32,
    pub columns: i32,

    pub screen_height: i32,
    pub lines: i32,

    pub screen_port: i32,
}
