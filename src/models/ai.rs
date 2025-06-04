use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Message {
    pub msg: String
}

#[derive(Serialize, Deserialize)]
pub struct AiResponse {
    pub ai_response: String,
}