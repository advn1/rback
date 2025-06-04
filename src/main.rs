use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    http::Method,
    routing::{get, post},
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
    handlers::auth::{login, logout, refresh, register},
    models::app::AppState,
};

use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

#[tokio::main]
async fn main() {
    let pool = connect_to_database().await;
    let connection_db = Arc::new(AppState {
        users_db: pool.clone(),
        tokens_db: pool.clone(), // Тот же pool
    });

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

    // build our application with a single route
    let app = Router::new()
        .route("/text", get(analyze_text).layer(ai_governor_layer))
        .route("/refresh", post(refresh))
        .layer(axum_middleware::from_fn(auth_middleware))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .layer(ServiceBuilder::new().layer(cors_layer))
        .layer(TraceLayer::new_for_http())
        .with_state(connection_db);

    let app: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
        app.into_make_service_with_connect_info();

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4006")
        .await
        .unwrap();

    println!("listening to 4006");

    axum::serve(listener, app).await.unwrap();
}
