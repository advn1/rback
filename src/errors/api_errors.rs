use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct GeminiApiErrorWrapper {
    pub error: GeminiApiError,
}

impl IntoResponse for GeminiApiErrorWrapper {
    fn into_response(self) -> axum::response::Response {
        let status =
            StatusCode::from_u16(self.error.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GeminiApiError {
    pub code: u16,
    pub message: String,
}


#[derive(Serialize)]
pub struct DatabaseError {
    pub error: String,
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}
use thiserror::Error;

use crate::utils::validation::ValidationError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Validation failed: {0:?}")]
    Validation(ValidationError),

    #[error("Validation failed: {0:?}")]
    Gemini(GeminiApiErrorWrapper)
}