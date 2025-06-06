use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Deserialize)]
pub struct Message {
    pub msg: String
}

#[derive(Serialize, Deserialize)]
pub struct AiResponse {
    pub ai_response: String,
}

#[derive(Serialize, Deserialize, Debug, FromRow)]
pub struct Conversation {
    id: i64,
    user_id: i64,
    title: String,
    created_at: i64,
    updated_at: i64
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
    token_count: i64
}

// "CREATE TABLE IF NOT EXISTS messages (
//     id INTEGER PRIMARY KEY AUTOINCREMENT,
//     conversation_id INTEGER NOT NULL,
//     role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
//     content TEXT NOT NULL,
//     timestamp INTEGER NOT NULL,
//     token_count INTEGER,
//     FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE

