use secrecy::{ExposeSecret, SecretString};
use sqlx::{Pool, Sqlite, SqlitePool};

pub struct AppState {
    pub users_db: Pool<Sqlite>,
    pub tokens_db: Pool<Sqlite>,
    salt: SecretString
}

impl AppState {
    pub fn new(users_db: SqlitePool, tokens_db: SqlitePool, salt: SecretString) -> Self {
        Self {
            users_db,
            tokens_db,
            salt,
        }
    }

    pub fn salt(&self) -> String {
        self.salt.expose_secret().to_string()
    }
}