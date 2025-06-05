use sqlx::{Pool, Sqlite};

pub struct AppState {
    pub users_db: Pool<Sqlite>,
    pub tokens_db: Pool<Sqlite>,
    pub salt: String
}