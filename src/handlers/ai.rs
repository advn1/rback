use std::env;

use axum::{debug_handler, Json};
use gemini_rust::{Error, Gemini};

use crate::{errors::api_errors::GeminiApiErrorWrapper, models::{ai::{AiResponse, Message}}};

#[debug_handler]
#[allow(unused)]
pub async fn analyze_text(
    Json(payload): Json<Message>,
) -> Result<Json<AiResponse>, GeminiApiErrorWrapper> {
    println!("There");

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