use secrecy::{ExposeSecret, SecretString};
use sqlx::{Pool, Sqlite, SqlitePool};

pub struct AppState {
    pub users_db: Pool<Sqlite>,
    pub tokens_db: Pool<Sqlite>,
    pub chat_db: Pool<Sqlite>,
    salt: SecretString,
    access_key: SecretString,
    refresh_key: SecretString
}

impl AppState {
    pub fn new(users_db: SqlitePool, tokens_db: SqlitePool, chat_db: SqlitePool, salt: SecretString, access_key: SecretString, refresh_key: SecretString) -> Self {
        Self {
            users_db,
            tokens_db,
            chat_db,
            salt,
            access_key,
            refresh_key
        }
    }

    pub fn get_salt(&self) -> String {
        self.salt.expose_secret().to_string()
    }

    pub fn get_access_key(&self) -> String {
        self.access_key.expose_secret().to_string()
    }

    pub fn get_refresh_key(&self) -> String {
        self.refresh_key.expose_secret().to_string()
    }
}