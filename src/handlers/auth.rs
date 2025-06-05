use argon2::{self, Config, hash_encoded, verify_encoded};
use std::{env, sync::Arc, vec};

use axum::{
    Extension, Json, debug_handler,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, prelude::FromRow};
use uuid::Uuid;
use validator::Validate;

use crate::{
    database::connection::{add_token, add_user},
    models::{
        app::AppState,
        auth::{DBToken, TokenClaims},
        user::{LoginData, OnSuccessRegister, RegisterData, UserDB},
    },
    utils::validation::{ValidationDetail, ValidationError, format_validation_errors},
};

#[derive(Deserialize, Serialize, FromRow)]
pub struct NewTokens {
    pub new_access_token: String,
    pub new_refresh_token: String,
}

#[derive(Deserialize, Serialize, FromRow, Debug)]
pub struct RefreshToken {
    pub refresh_token: String,
}

#[allow(unused)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterData>,
) -> Result<Json<OnSuccessRegister>, ValidationError> {
    if let Err(validation_errors) = payload.validate() {
        return Err(format_validation_errors(validation_errors));
    }

    let user_exists: Option<UserDB> =
        sqlx::query_as("SELECT * FROM users WHERE name = (?1) OR email = (?2)")
            .bind(&payload.name)
            .bind(&payload.email)
            .fetch_optional(&state.users_db)
            .await
            .map_err(|e| ValidationError {
                error: "Database error".to_string(),
                details: vec![ValidationDetail {
                    field: "database".to_string(),
                    messages: vec![format!("Database query failed: {}", e)],
                }],
            })?;

    if user_exists.is_some() {
        return Err(ValidationError {
            error: "Validation failed".to_string(),
            details: vec![ValidationDetail {
                field: "user".to_string(),
                messages: vec!["User with this name or email already exists".to_string()],
            }],
        });
    }

    let hashed_password = hash_encoded(
        &payload.password.as_bytes(),
        &state.salt().as_bytes(),
        &Config::default(),
    )
    .map_err(|e| ValidationError {
        error: "Internal error".to_string(),
        details: vec![ValidationDetail {
            field: "password".to_string(),
            messages: vec![format!("Failed to hash password: {}", e)],
        }],
    })?;

    let user = add_user(
        &payload.name,
        &hashed_password,
        &payload.email,
        &state.users_db,
    )
    .await
    .map_err(|e| ValidationError {
        error: "Database error".to_string(),
        details: vec![ValidationDetail {
            field: "database".to_string(),
            messages: vec![format!("Failed to create user: {}", e)],
        }],
    })?;

    Ok(user)
}

#[derive(Serialize)]
pub struct Tokens {
    access_token: String,
    refresh_token: String,
}

#[allow(unused)]
#[debug_handler]
pub async fn login(
    State(state): State<Arc<AppState>>,
    req: HeaderMap,
    Json(payload): Json<LoginData>,
) -> Result<Json<Tokens>, (StatusCode, ValidationError)> {
    if let Some(header_value) = req.get("Authorization") {
        if let Ok(header_str) = header_value.to_str() {
            if header_str.starts_with("Bearer ") {
                return Err((
                    StatusCode::CONFLICT,
                    ValidationError {
                        error: "Authorization error".to_string(),
                        details: vec![ValidationDetail {
                            field: "Authorization".to_string(),
                            messages: vec!["Already authorized".to_string()],
                        }],
                    },
                ));
            } else {
                return Err((
                    StatusCode::CONFLICT,
                    ValidationError {
                        error: "Authorization error".to_string(),
                        details: vec![ValidationDetail {
                            field: "Authorization".to_string(),
                            messages: vec!["Not bearer".to_string()],
                        }],
                    },
                ));
            }
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                ValidationError {
                    error: "Authorization error".to_string(),
                    details: vec![ValidationDetail {
                        field: "Authorization".to_string(),
                        messages: vec!["Header not valid UTF-8".to_string()],
                    }],
                },
            ));
        }
    }

    let user_result: Result<UserDB, sqlx::Error> =
        sqlx::query_as("SELECT * FROM users WHERE email = ?")
            .bind(&payload.email)
            .fetch_one(&state.users_db)
            .await;

    let user = match user_result {
        Ok(u) => u,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ValidationError {
                    error: "Database query failed".to_string(),
                    details: vec![ValidationDetail {
                        field: "email".to_string(),
                        messages: vec![format!("{}", e)],
                    }],
                },
            ));
        }
    };

    let is_correct = verify_encoded(&user.password, &payload.password.as_bytes()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            ValidationError {
                error: "User authentication failed".to_string(),
                details: vec![ValidationDetail {
                    field: "credentials".to_string(),
                    messages: vec!["Invalid email or password".to_string()],
                }],
            },
        )
    })?;

    if is_correct {
        let claims = TokenClaims {
            user_id: user.id,
            email: user.email.clone(),
            name: user.name.clone(),
            exp: (Utc::now() + Duration::minutes(5)).timestamp(),
            token_type: "Access".to_string(),
            used: false,
            jti: Uuid::new_v4().to_string(),
        };

        let access_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(
                env::var("SECRET_KEY_ACCESS")
                    .expect("No secret key is provided")
                    .as_ref(),
            ),
        )
        .unwrap();

        let claims_refresh = TokenClaims {
            // Renamed to avoid confusion
            user_id: user.id,
            email: user.email.clone(),
            name: user.name.clone(),
            exp: (Utc::now() + Duration::days(7)).timestamp(),
            token_type: "Refresh".to_string(),
            used: false, // This 'used' is for the claim itself, not DB state initially
            jti: Uuid::new_v4().to_string(),
        };

        let refresh_token = encode(
            &Header::default(),
            &claims_refresh,
            &EncodingKey::from_secret(
                env::var("SECRET_KEY_REFRESH")
                    .expect("No secret key was provided")
                    .as_ref(),
            ),
        )
        .unwrap();

        let hashed_refresh_token = argon2::hash_encoded(
            refresh_token.as_bytes(),
            &state.salt().as_bytes(),
            &Config::default(),
        )
        .unwrap();

        let _ = add_token(&claims_refresh, &hashed_refresh_token, &state.tokens_db)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ValidationError {
                        error: "Database error".to_string(),
                        details: vec![ValidationDetail {
                            field: "database".to_string(),
                            messages: vec![format!("Failed to add token: {}", e)],
                        }],
                    },
                )
            })?;

        Ok(Json(Tokens {
            access_token,
            refresh_token,
        }))
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            ValidationError {
                error: "Authentication failed".to_string(),
                details: vec![ValidationDetail {
                    field: "credentials".to_string(),
                    messages: vec!["Wrong password or email".to_string()],
                }],
            },
        ))
    }
}

#[allow(unused)]
#[debug_handler]
pub async fn refresh(
    Extension(user_data): Extension<TokenClaims>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RefreshToken>,
) -> Result<Json<NewTokens>, ValidationError> {
    // Validate input
    if payload.refresh_token.trim().is_empty() {
        return Err(ValidationError {
            error: "Invalid refresh token".to_string(),
            details: vec![ValidationDetail {
                field: "refresh_token".to_string(),
                messages: vec!["Refresh token cannot be empty".to_string()],
            }],
        });
    }

    let tokens: Vec<DBToken> =
        match sqlx::query_as("SELECT * FROM tokens WHERE user_id = ? AND used = FALSE")
            .bind(&user_data.user_id)
            .fetch_all(&state.tokens_db)
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                return Err(ValidationError {
                    error: "Database error".to_string(),
                    details: vec![ValidationDetail {
                        field: "database".to_string(),
                        messages: vec!["Failed to fetch user tokens".to_string()],
                    }],
                });
            }
        };

    let matched_token = find_matching_token(&tokens, &payload.refresh_token)?;

    let (new_access_token, new_refresh_token, new_refresh_claims) =
        generate_new_tokens(&user_data).await?;

    update_tokens_in_database(
        &state.tokens_db,
        &matched_token,
        &new_refresh_claims,
        &new_refresh_token,
        &state.salt()
    )
    .await?;

    Ok(Json(NewTokens {
        new_access_token,
        new_refresh_token,
    }))
}

fn find_matching_token(
    tokens: &[DBToken],
    refresh_token: &str,
) -> Result<DBToken, ValidationError> {
    for token in tokens {
        match argon2::verify_encoded(&token.token, refresh_token.as_bytes()) {
            Ok(true) => {
                return Ok(token.clone());
            }
            _ => continue,
        }
    }

    Err(ValidationError {
        error: "Invalid refresh token".to_string(),
        details: vec![ValidationDetail {
            field: "refresh_token".to_string(),
            messages: vec!["The provided refresh token is invalid or expired".to_string()],
        }],
    })
}

async fn generate_new_tokens(
    user_data: &TokenClaims,
) -> Result<(String, String, TokenClaims), ValidationError> {
    let access_secret = env::var("SECRET_KEY_ACCESS").map_err(|_| ValidationError {
        error: "Configuration error".to_string(),
        details: vec![ValidationDetail {
            field: "configuration".to_string(),
            messages: vec!["Access token secret not configured".to_string()],
        }],
    })?;

    let refresh_secret = env::var("SECRET_KEY_REFRESH").map_err(|_| ValidationError {
        error: "Configuration error".to_string(),
        details: vec![ValidationDetail {
            field: "configuration".to_string(),
            messages: vec!["Refresh token secret not configured".to_string()],
        }],
    })?;

    let new_access_claims = TokenClaims {
        name: user_data.name.clone(),
        email: user_data.email.clone(),
        user_id: user_data.user_id,
        exp: (Utc::now() + Duration::minutes(5)).timestamp(),
        token_type: "Access".to_string(),
        used: false,
        jti: Uuid::new_v4().to_string(),
    };

    let new_access_token = jsonwebtoken::encode(
        &Header::default(),
        &new_access_claims,
        &EncodingKey::from_secret(access_secret.as_ref()),
    )
    .map_err(|e| ValidationError {
        error: "Token generation failed".to_string(),
        details: vec![ValidationDetail {
            field: "access_token".to_string(),
            messages: vec![format!("Failed to generate access token: {}", e)],
        }],
    })?;

    let new_refresh_claims = TokenClaims {
        name: user_data.name.clone(),
        email: user_data.email.clone(),
        user_id: user_data.user_id,
        exp: (Utc::now() + Duration::days(7)).timestamp(),
        token_type: "Refresh".to_string(),
        used: false,
        jti: Uuid::new_v4().to_string(),
    };

    let new_refresh_token = jsonwebtoken::encode(
        &Header::default(),
        &new_refresh_claims,
        &EncodingKey::from_secret(refresh_secret.as_ref()),
    )
    .map_err(|e| ValidationError {
        error: "Token generation failed".to_string(),
        details: vec![ValidationDetail {
            field: "refresh_token".to_string(),
            messages: vec![format!("Failed to generate refresh token: {}", e)],
        }],
    })?;

    Ok((new_access_token, new_refresh_token, new_refresh_claims))
}

async fn update_tokens_in_database(
    db: &Pool<Sqlite>,
    matched_token: &DBToken,
    new_refresh_claims: &TokenClaims,
    new_refresh_token: &str,
    salt: &str
) -> Result<(), ValidationError> {
    sqlx::query("UPDATE tokens SET used = TRUE WHERE token = ?")
        .bind(&matched_token.token)
        .execute(db)
        .await
        .map_err(|e| ValidationError {
            error: "Database error".to_string(),
            details: vec![ValidationDetail {
                field: "database".to_string(),
                messages: vec![format!("Failed to invalidate old token: {}", e)],
            }],
        })?;


    let hashed_refresh_token = argon2::hash_encoded(
        new_refresh_token.as_bytes(),
        &salt.as_bytes(),
        &Config::default(),
    )
    .map_err(|e| ValidationError {
        error: "Token processing error".to_string(),
        details: vec![ValidationDetail {
            field: "refresh_token".to_string(),
            messages: vec![format!("Failed to process refresh token: {}", e)],
        }],
    })?;

    let _ = add_token(new_refresh_claims, &hashed_refresh_token, db)
        .await
        .map_err(|e| ValidationError {
            error: "Database error".to_string(),
            details: vec![ValidationDetail {
                field: "database".to_string(),
                messages: vec![format!("Failed to store new refresh token: {}", e)],
            }],
        })?;

    Ok(())
}

#[allow(unused)]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Json(paylod): Json<RefreshToken>,
) -> Result<(), ValidationError> {
    let hashed_refresh_token = argon2::hash_encoded(
        paylod.refresh_token.as_bytes(),
        &state.salt().as_bytes(),
        &Config::default(),
    )
    .map_err(|e| ValidationError {
        error: "Token processing error".to_string(),
        details: vec![ValidationDetail {
            field: "refresh_token".to_string(),
            messages: vec!["Failed to process refresh token".to_string()],
        }],
    })?;

    let _ = sqlx::query("DELETE FROM tokens WHERE token = ?")
        .bind(&hashed_refresh_token)
        .execute(&state.tokens_db)
        .await
        .map_err(|e| ValidationError {
            error: "Database error".to_string(),
            details: vec![ValidationDetail {
                field: "database".to_string(),
                messages: vec!["Failed to delete refresh token".to_string()],
            }],
        })?;

    Ok(())
}
