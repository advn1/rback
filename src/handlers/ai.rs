use std::{env, sync::Arc, time::Duration};

use axum::{
    Extension, Json, debug_handler,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::Response,
};
use chrono::Utc;
use gemini_rust::{Error, Gemini};
use serde::Deserialize;

use crate::{
    database::connection::insert_chat_message_to_db,
    errors::api_errors::GeminiApiErrorWrapper,
    models::{
        ai::{AiResponse, ConvMessage, Conversation, Message as UserText, Title, UserMessage},
        app::AppState,
        auth::TokenClaims,
    },
    utils::validation::{ValidationDetail, ValidationError},
};

#[debug_handler]
#[allow(unused)]
pub async fn analyze_text(
    Json(payload): Json<UserText>,
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
) -> Result<Json<Conversation>, ValidationError> {
    let time_now = Utc::now().timestamp();
    let _ = sqlx::query("INSERT INTO conversations (user_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)")
        .bind(user_data.user_id)
        .bind("New chat")
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

    let r: Conversation = sqlx::query_as("SELECT * FROM conversations where user_id = ? AND created_at = ?")
        .bind(user_data.user_id)
        .bind(time_now)
        .fetch_one(&state.chat_db)
        .await
        .unwrap();

    println!("{:?}", r);

    Ok(Json(r))
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
    pub id: i64,
}

pub async fn get_user_conversations_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Conversation>>, ValidationError> {
    let r: Vec<Conversation> =
        sqlx::query_as("SELECT * FROM conversations WHERE user_id = (?1) AND id = (?2)")
            .bind(user_data.user_id)
            .bind(id)
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
    Path(id): Path<i64>,
    Json(payload): Json<Title>,
) -> Result<Json<Conversation>, ValidationError> {
    let existing: Option<Conversation> =
        sqlx::query_as("SELECT * FROM conversations WHERE user_id = ?1 AND id = ?2")
            .bind(user_data.user_id)
            .bind(id)
            .fetch_optional(&state.chat_db)
            .await
            .map_err(|e| ValidationError {
                error: "Database query failed".to_string(),
                details: vec![ValidationDetail {
                    field: "id".to_string(),
                    messages: vec![format!("Check existence failed: {}", e)],
                }],
            })?;

    if existing.is_none() {
        return Err(ValidationError {
            error: "Conversation not found".to_string(),
            details: vec![ValidationDetail {
                field: "id".to_string(),
                messages: vec!["No conversation with this ID for the current user.".to_string()],
            }],
        });
    }

    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
    )
    .bind(&payload.title)
    .bind(now)
    .bind(id)
    .bind(user_data.user_id)
    .execute(&state.chat_db)
    .await
    .map_err(|e| ValidationError {
        error: "Database update failed".to_string(),
        details: vec![ValidationDetail {
            field: "update".to_string(),
            messages: vec![format!("Failed to update: {}", e)],
        }],
    })?;

    let updated: Conversation =
        sqlx::query_as("SELECT * FROM conversations WHERE id = ?1 AND user_id = ?2")
            .bind(id)
            .bind(user_data.user_id)
            .fetch_one(&state.chat_db)
            .await
            .map_err(|e| ValidationError {
                error: "Fetch updated conversation failed".to_string(),
                details: vec![ValidationDetail {
                    field: "query".to_string(),
                    messages: vec![format!("Failed to fetch after update: {}", e)],
                }],
            })?;

    Ok(Json(updated))
}

pub async fn delete_conversation_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ValidationError> {
    let result = sqlx::query("DELETE FROM conversations WHERE id = ?1 AND user_id = ?2")
        .bind(id)
        .bind(user_data.user_id)
        .execute(&state.chat_db)
        .await
        .map_err(|e| ValidationError {
            error: "Database delete failed".to_string(),
            details: vec![ValidationDetail {
                field: "id".to_string(),
                messages: vec![format!("Error deleting conversation: {}", e)],
            }],
        })?;

    if result.rows_affected() == 0 {
        return Err(ValidationError {
            error: "Not found".to_string(),
            details: vec![ValidationDetail {
                field: "id".to_string(),
                messages: vec!["No conversation with this ID for the current user.".to_string()],
            }],
        });
    }

    Ok(StatusCode::NO_CONTENT)
}

#[debug_handler]
pub async fn delete_message_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Path((conversation_id, message_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ValidationError> {
    let conversation_exists =
        sqlx::query_scalar::<_, i64>("SELECT 1 FROM conversations WHERE id = ?1 AND user_id = ?2")
            .bind(conversation_id)
            .bind(user_data.user_id)
            .fetch_optional(&state.chat_db)
            .await
            .map_err(|e| ValidationError {
                error: "Database check failed".to_string(),
                details: vec![ValidationDetail {
                    field: "conversation_id".to_string(),
                    messages: vec![format!("Conversation check failed: {}", e)],
                }],
            })?;

    if conversation_exists.is_none() {
        return Err(ValidationError {
            error: "Conversation not found or unauthorized".to_string(),
            details: vec![ValidationDetail {
                field: "conversation_id".to_string(),
                messages: vec!["No conversation with this ID for the current user.".to_string()],
            }],
        });
    }

    let result = sqlx::query("DELETE FROM messages WHERE conversation_id = ?1 AND timestamp = ?2")
        .bind(conversation_id)
        .bind(message_id)
        .execute(&state.chat_db)
        .await
        .map_err(|e| ValidationError {
            error: "Message deletion failed".to_string(),
            details: vec![ValidationDetail {
                field: "message_id".to_string(),
                messages: vec![format!("Failed to delete message: {}", e)],
            }],
        })?;

    if result.rows_affected() == 0 {
        return Err(ValidationError {
            error: "Message not found".to_string(),
            details: vec![ValidationDetail {
                field: "message_id".to_string(),
                messages: vec!["No message with this ID in the conversation.".to_string()],
            }],
        });
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

pub async fn get_conversation_messages_by_id(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Path(conversation_id): Path<i64>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Vec<ConvMessage>>, ValidationError> {
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(10);

    if page == 0 || limit == 0 {
        return Err(ValidationError {
            error: "Invalid pagination parameters".into(),
            details: vec![
                ValidationDetail {
                    field: "page".into(),
                    messages: if page == 0 { vec!["Page must be greater than 0".into()] } else { vec![] },
                },
                ValidationDetail {
                    field: "limit".into(),
                    messages: if limit == 0 { vec!["Limit must be greater than 0".into()] } else { vec![] },
                },
            ],
        });
    }

    let offset = (page - 1) * limit;

    let result = sqlx::query_as::<_, ConvMessage>(
        "SELECT * FROM messages WHERE conversation_id = ? LIMIT ? OFFSET ?",
    )
    .bind(conversation_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.chat_db)
    .await;

    match result {
        Ok(messages) => Ok(Json(messages)),
        Err(e) => Err(ValidationError {
            error: "Database query failed".into(),
            details: vec![ValidationDetail {
                field: "database".into(),
                messages: vec![format!("Failed to fetch conversation messages: {}", e)],
            }],
        }),
    }
}

#[debug_handler]
pub async fn post_user_message(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
    Query(params): Query<UserMessage>,
) -> Response {
    println!("there");
    ws.on_upgrade(move |socket| handle_user_message(socket, params, state))
}

async fn handle_user_message(mut socket: WebSocket, params: UserMessage, state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            let r = insert_chat_message_to_db(
                "user", // shitty code
                params.conversation_id,
                msg.to_text().unwrap(),
                &state.chat_db,
            )
            .await;

            if let Err(e) = r {
                let _ = socket.send(e.into()).await;
            }

            let key = env::var("GEMINI_API_KEY").expect("API key was not provided");
            let client = Gemini::new(key);
            let gemini_response = async {
                let response = client
                    .generate_content()
                    .with_user_message(msg.to_text().unwrap())
                    .execute()
                    .await;

                match response {
                    Ok(_) => {}
                    Err(e) => {
                        let json_start = e.to_string().find("{").expect("Not a pure json");
                        let new_e: GeminiApiErrorWrapper =
                            serde_json::from_str(&e.to_string()[json_start..])
                                .expect("Incorrect GeminiApiError json");

                        let stringified = serde_json::to_string(&new_e).unwrap_or_else(|_| {
                            "{\"error\": \"Internal server error\"}".to_string() //shit
                        });

                        return Err(stringified);
                    }
                }

                let response = response.unwrap();

                enum ResponseStatus {
                    NotReady,
                    Ready,
                }

                Ok((ResponseStatus::Ready, response))
            };

            let typing = async {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let _ = socket.send("typing".into()).await;
                }
            };

            let result: Result<String, Message> = tokio::select! {
                res = gemini_response => match res {
                    Ok((_, response)) => {
                        let response_text = response.text();
                        Ok(response_text)
                    },
                    Err(e) => Err(e.into()),
                },
                never = typing => match never {}
            };

            match result {
                Ok(response_text) => {
                    let r = insert_chat_message_to_db(
                        "assistant",
                        params.conversation_id,
                        &response_text,
                        &state.chat_db,
                    )
                    .await;

                    if let Err(e) = r {
                        let _ = socket.send(e.into()).await;
                    }

                    let _ = socket.send(Message::from(response_text)).await;
                }
                Err(err_msg) => {
                    let _ = socket.send(err_msg).await;
                }
            }
        } else {
            // client disconnected
        };
    }
}
