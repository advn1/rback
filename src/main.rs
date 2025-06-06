use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, mpsc::channel},
    thread,
    time::Duration,
};

use axum::{
    Json, Router, debug_handler,
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Utf8Bytes, WebSocket},
    },
    http::Method,
    response::{IntoResponse, Response},
    routing::{any, get, post},
};

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::middleware as axum_middleware;

mod models;

mod errors;

mod database;

mod middleware;
use chrono::Utc;
use gemini_rust::{Gemini, GenerationResponse};
use middleware::auth::auth_middleware;

mod handlers;
use handlers::ai::analyze_text;
use serde::Deserialize;
use tower::{ServiceBuilder, layer::util::Stack};
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor,
};
mod utils;

use crate::{
    database::connection::connect_to_database,
    errors::api_errors::{AppError, GeminiApiError, GeminiApiErrorWrapper},
    handlers::{
        ai::{
            create_conversation, get_user_conversations, get_user_conversations_by_id,
            update_conversation_by_id,
        },
        auth::{login, logout, refresh, register},
    },
    models::{ai::ConvMessage, app::AppState},
    utils::validation::{ValidationDetail, ValidationError},
};

use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

#[tokio::main]
async fn main() {
    let pool = connect_to_database().await;

    let salt = env::var("SALT").expect("Salt was not provided");
    let access_key = env::var("SECRET_KEY_ACCESS").expect("Secret key was not provided");
    let refresh_key = env::var("SECRET_KEY_REFRESH").expect("Refresh key was not provided");

    let connection_db = Arc::new(AppState::new(
        pool.clone(),
        pool.clone(),
        pool.clone(),
        salt.into(),
        access_key.into(),
        refresh_key.into(),
    ));

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(5)
            .key_extractor(PeerIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let ai_governor_layer = GovernorLayer {
        config: governor_conf,
    };

    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![Method::POST, Method::GET]);

    let app = Router::new()
        .route("/text", get(analyze_text).layer(ai_governor_layer))
        .route(
            "/conversations",
            get(get_user_conversations).post(create_conversation),
        )
        // .route(
        //     "/conversations/{id}",
        //     get(get_user_conversations_by_id).put(update_conversation_by_id),
        // )
        .route("/conversations_ws", any(post_user_message))
        .layer(axum_middleware::from_fn(auth_middleware))
        .route("/refresh", post(refresh))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .layer(ServiceBuilder::new().layer(cors_layer))
        .layer(TraceLayer::new_for_http())
        .with_state(connection_db);

    let app: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
        app.into_make_service_with_connect_info();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4006")
        .await
        .unwrap();

    println!("listening to 4006");

    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize, Debug)]
struct UserMessage {
    conversation_id: i64,
}

#[debug_handler]
async fn post_user_message(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
    Query(params): Query<UserMessage>,
) -> Response {
    ws.on_upgrade(move |socket| handle_user_message(socket, params, state))
}

async fn handle_user_message(mut socket: WebSocket, params: UserMessage, state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
        let msg = if let Ok(msg) = msg {
            let insert = sqlx::query(
                "INSERT INTO messages (conversation_id, role, content, timestamp, token_count)
VALUES (?1, 'user', ?2, ?3, 4)",
            )
            .bind(&params.conversation_id)
            .bind(msg.to_text().unwrap())
            .bind(Utc::now().timestamp())
            .execute(&state.chat_db)
            .await;

            if let Err(e) = insert {
                let stringified = serde_json::to_string(&ValidationError {
                    error: "Database query failed".to_string(),
                    details: vec![ValidationDetail {
                        field: "database".to_string(),
                        messages: vec!["adding user message to database failed".to_string()],
                    }],
                })
                .unwrap_or_else(|_| "{\"error\": \"Internal server error\"}".to_string());

                let _ = socket.send(axum::extract::ws::Message::Text(stringified.into())).await;
            }

            // let r: Vec<ConvMessage> = sqlx::query_as("SELECT * FROM messages")
            //     .fetch_all(&state.chat_db)
            //     .await
            //     .unwrap();

            // println!("WS ON ADDING USER MESSAGE {:?}", r);

            let key = env::var("GEMINI_API_KEY").expect("API key was not provided");

            let client = Gemini::new(key);

            enum ResponseStatus {
                NotReady,
                Ready,
            }

            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            let (txx, mut rxx) = tokio::sync::mpsc::channel(1);

            let state = state.clone();
            tokio::spawn(async move {
                let response = client
                    .generate_content()
                    .with_user_message(msg.to_text().unwrap())
                    .execute()
                    .await;

                if let Err(e) = &response {
                    let json_start = e.to_string().find("{").expect("Not a pure json");
                    let new_e: GeminiApiErrorWrapper =
                        serde_json::from_str(&e.to_string()[json_start..])
                            .expect("Incorrect GeminiApiError json");

                    let stringified = serde_json::to_string(&new_e).unwrap_or_else(|_| "{\"error\": \"Internal server error\"}".to_string());                    
                }

                let insert = sqlx::query(
                    "INSERT INTO messages (conversation_id, role, content, timestamp, token_count)
        VALUES (?1, 'assistant', ?2, ?3, 4)",
                )
                .bind(&params.conversation_id)
                .bind(&response.unwrap().clone().text())
                .bind(Utc::now().timestamp())
                .execute(&state.chat_db)
                .await;

                if let Err(e) = insert {
                    let stringified = serde_json::to_string(&ValidationError {
                        error: "Database query failed".to_string(),
                        details: vec![ValidationDetail {
                            field: "database".to_string(),
                            messages: vec!["adding assistant message to database failed".to_string()],
                        }],
                    })
                    .unwrap_or_else(|_| "{\"error\": \"Internal server error\"}".to_string());
    
                }
                // let r: Vec<ConvMessage> = sqlx::query_as("SELECT * FROM messages")
                // .fetch_all(&state.chat_db)
                // .await
                // .unwrap();

                // println!("WS ON ADDING ASSISTANT MESSAGE {:?}", r);

                tx.send(ResponseStatus::Ready).await;
                txx.send(response.unwrap()).await;
            });

            loop {
                tokio::select! {
                    Some(r) = rxx.recv() => {
                        socket.send(r.text().into()).await;
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        socket.send("typing..".into()).await;
                    }
                }
            }
        } else {
            // client disconnected
        };
    }
}
