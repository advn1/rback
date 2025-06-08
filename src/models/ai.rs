use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Deserialize)]
pub struct Message {
    pub msg: String,
}

#[derive(Serialize, Deserialize)]
pub struct AiResponse {
    pub ai_response: String,
}

#[derive(Serialize, Deserialize, Debug, FromRow)]
pub struct Conversation {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl IntoResponse for Conversation {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Serialize, Deserialize, Debug, FromRow)]
pub struct ConvMessage {
    conversation_id: i64,
    role: String,
    content: String,
    timestamp: i64,
    token_count: i64,
}

#[derive(Deserialize, Debug)]
pub struct UserMessage {
    pub conversation_id: i64,
}

//For updating conversation title
#[derive(Deserialize)]
pub struct Title {
    pub title: String
}