use std::{env, sync::Arc};

use axum::{debug_handler, extract::{Query, State}, Extension, Json};
use chrono::Utc;
use gemini_rust::{Error, Gemini};
use serde::Deserialize;

use crate::{
    errors::api_errors::GeminiApiErrorWrapper,
    models::{
        ai::{AiResponse, Conversation, Message},
        app::AppState,
        auth::TokenClaims,
    },
    utils::validation::{ValidationDetail, ValidationError},
};

#[debug_handler]
#[allow(unused)]
pub async fn analyze_text(
    Json(payload): Json<Message>,
) -> Result<Json<AiResponse>, GeminiApiErrorWrapper> {
    let text = make_request_to_ai(&payload.msg).await;

    match text {
        Ok(text) => return Ok(Json(text)),
        Err(e) => match e {
            _ => {
                let json_start = e.to_string().find("{").expect("Not a pure json");
                let new_e: GeminiApiErrorWrapper =
                    serde_json::from_str(&e.to_string()[json_start..])
                        .expect("Incorrect GeminiApiError json");
                return Err(new_e);
            }
        },
    }
}

pub async fn make_request_to_ai(msg: &str) -> Result<AiResponse, Error> {
    let key = env::var("GEMINI_API_KEY").unwrap();

    let client = Gemini::new(key);

    let response = client
        .generate_content()
        .with_user_message(msg)
        .execute()
        .await?;

    return Ok(AiResponse {
        ai_response: response.text(),
    });
}
pub async fn create_conversation(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
) -> Result<(), ValidationError> {
    let time_now = Utc::now().timestamp();
    let _ = sqlx::query("INSERT INTO conversations (user_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(user_data.user_id)
        .bind("new conversation")
        .bind(time_now)
        .bind(time_now)
        .execute(&state.chat_db)
        .await.map_err(|e| ValidationError {
            error: "Database query failed".to_string(),
            details: vec![ValidationDetail {
                field: "credentials".to_string(),
                messages: vec![format!("creating new conversation failed: {}",e)]
            }]
        })?;

    let r: Vec<Conversation> = sqlx::query_as("SELECT * FROM conversations")
        .fetch_all(&state.chat_db)
        .await
        .unwrap();

    println!("{:?}", r);

    Ok(())
}

#[debug_handler]
pub async fn get_user_conversations(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Conversation>>, ValidationError> {
    let r: Vec<Conversation> = sqlx::query_as("SELECT * FROM conversations where user_id = ?")
        .bind(user_data.user_id)
        .fetch_all(&state.chat_db)
        .await
        .map_err(|e| ValidationError {
            error: "Database query failed".to_string(),
            details: vec![ValidationDetail {
                field: "credentials".to_string(),
                messages: vec![format!("getting users conversations failed: {}", e)],
            }],
        })?;
    
    Ok(Json(r))
}

#[derive(Deserialize)]
pub struct ConversationID {
    pub id: i64
}

pub async fn get_user_conversations_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ConversationID>
) -> Result<Json<Vec<Conversation>>, ValidationError> {
    let r: Vec<Conversation> = sqlx::query_as("SELECT * FROM conversations where user_id = (?1) AND id = (?2)")
        .bind(user_data.user_id)
        .bind(query.id)
        .fetch_all(&state.chat_db)
        .await
        .map_err(|e| ValidationError {
            error: "Database query failed".to_string(),
            details: vec![ValidationDetail {
                field: "credentials".to_string(),
                messages: vec![format!("getting user's conversations by id failed: {}", e)],
            }],
        })?;
    
    Ok(Json(r))
}

pub async fn update_conversation_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ConversationID>,
    Json(payload): Json<Conversation>
) -> Result<Json<Vec<Conversation>>, ValidationError> {
    // let what_update = vec![];

    let r: Vec<Conversation> = sqlx::query_as("SELECT * FROM conversations where user_id = (?1) AND id = (?2)")
        .bind(user_data.user_id)
        .bind(query.id)
        .fetch_all(&state.chat_db)
        .await
        .map_err(|e| ValidationError {
            error: "Database query failed".to_string(),
            details: vec![ValidationDetail {
                field: "credentials".to_string(),
                messages: vec![format!("getting user's conversations by id failed: {}", e)],
            }],
        })?;
    
    Ok(Json(r))
}
