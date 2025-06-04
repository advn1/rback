pub mod validation {
    use axum::{http::StatusCode, response::IntoResponse, Json};
    use serde::Serialize;
    use validator::ValidationErrors;

    #[derive(Serialize)]
    pub struct ValidationError {
        pub error: String,
        pub details: Vec<ValidationDetail>,
    }

    #[derive(Serialize)]
    pub struct ValidationDetail {
        pub field: String,
        pub messages: Vec<String>,
    }

    impl IntoResponse for ValidationError {
        fn into_response(self) -> axum::response::Response {
            (StatusCode::BAD_REQUEST, Json(self)).into_response()
        }
    }

    pub fn format_validation_errors(errors: ValidationErrors) -> ValidationError {
        let mut details = Vec::new();

        for (field, field_errors) in errors.field_errors() {
            let messages: Vec<String> = field_errors
                .iter()
                .map(|error| {
                    error
                        .message
                        .as_ref()
                        .map(|msg| msg.to_string())
                        .unwrap_or_else(|| format!("Invalid value for field '{}'", field))
                })
                .collect();

            details.push(ValidationDetail {
                field: field.to_string(),
                messages,
            });
        }

        ValidationError {
            error: "Validation failed".to_string(),
            details,
        }
    }
}
