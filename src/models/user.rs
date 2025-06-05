use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use validator::Validate;

#[derive(FromRow, Debug)]
pub struct UserDB {
    pub id: i64,
    pub name: String,
    pub password: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Validate, Debug)]
pub struct RegisterData {
    #[validate(length(
        min = 3,
        max = 48,
        message = "Name must be between 3 and 48 characters"
    ))]
    pub name: String,

    #[validate(
        length(
            min = 8,
            max = 128,
            message = "Password must be between 8 and 128 characters"
        ),
        custom(
            function = "validate_password_strength",
            message = "Password must contain at least one uppercase letter, one lowercase letter, one digit, and one special character"
        )
    )]
    pub password: String,

    #[validate(
        email(message = "Invalid email format"),
        length(max = 254, message = "Email is too long")
    )]
    pub email: String,
}

fn validate_password_strength(password: &str) -> Result<(), validator::ValidationError> {
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password
        .chars()
        .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c));

    if has_upper && has_lower && has_digit && has_special {
        Ok(())
    } else {
        Err(validator::ValidationError::new("weak_password"))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginData {
    pub password: String,
    pub email: String,
}

#[derive(Deserialize, Serialize)]
pub struct OnSuccessRegister {
    pub message: String,
    pub user_id: i64,
}
