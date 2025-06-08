use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    http::Method,
    routing::{any, delete, get, post},
};

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::middleware as axum_middleware;

mod models;

mod errors;

mod database;

mod middleware;
use middleware::auth::auth_middleware;

mod handlers;
use handlers::ai::analyze_text;
use tower::ServiceBuilder;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor,
};
mod utils;

use crate::{
    database::connection::connect_to_database,
    handlers::{
        ai::{
            create_conversation, delete_conversation_by_id, delete_message_by_id,
            get_conversation_messages_by_id, get_user_conversations, get_user_conversations_by_id,
            post_user_message, update_conversation_by_id,
        },
        auth::{login, logout, refresh, register},
    },
    models::app::AppState,
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
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/text", get(analyze_text).layer(ai_governor_layer))
        .route(
            "/conversations",
            get(get_user_conversations).post(create_conversation),
        )
        .route(
            "/conversations/{id}",
            get(get_user_conversations_by_id)
                .put(update_conversation_by_id)
                .delete(delete_conversation_by_id),
        )
        .route(
            "/conversations/{id}/messages/{message_id}",
            delete(delete_message_by_id),
        )
        .route(
            "/conversations/{id}/messages",
            get(get_conversation_messages_by_id),
        )
        .layer(axum_middleware::from_fn(auth_middleware))
        .route("/refresh", post(refresh))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/conversations_ws", get(post_user_message))

        .layer(ServiceBuilder::new().layer(cors_layer))
        .with_state(connection_db);

    let app: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
        app.into_make_service_with_connect_info();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4006")
        .await
        .unwrap();

    println!("listening to 4006");

    axum::serve(listener, app).await.unwrap();
}
