use std::env;

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};

use crate::models::auth::TokenClaims;

#[allow(unused)]
pub async fn auth_middleware(
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer") {
        println!("ERROR: Header doesn't start with Bearer");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    let validation = Validation::new(Algorithm::HS256);

    let access_key = env::var("SECRET_KEY_ACCESS").expect("SECRET_KEY_ACCESS not provided");

    let user_token: TokenData<TokenClaims> = decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(access_key.as_ref()),
        &validation,
    )
    .map_err(|e| {
        println!("FINAL ERROR: {:?}", e);
        StatusCode::UNAUTHORIZED
    })?;


    req.extensions_mut().insert(user_token.claims);
    Ok(next.run(req).await)
}
