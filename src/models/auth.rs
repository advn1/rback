use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokenClaims {
    pub name: String,
    pub email: String,
    pub user_id: i64,
    pub exp: i64,
    pub token_type: String,
    pub used: bool,
    pub jti: String
}

#[derive(Serialize, Deserialize, Clone, FromRow, Debug)]
pub struct DBToken {
    pub id: i64,
    pub token: String,
    pub name: String,
    pub email: String,
    pub user_id: i64,
    pub exp: i64,
    pub used: bool
}


