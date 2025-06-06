use axum::Json;
use serde::Serialize;
use sqlx::{Executor, Pool, Sqlite, sqlite};

use crate::models::{
    auth::TokenClaims,
    user::{OnSuccessRegister, UserDB},
};

pub async fn add_user(
    name: &str,
    password: &str,
    email: &str,
    conn: &Pool<Sqlite>,
) -> Result<Json<OnSuccessRegister>, sqlx::Error> {
    let r: Vec<UserDB> = sqlx::query_as("SELECT * FROM users")
        .fetch_all(conn)
        .await?;
    println!("all users: {:?}", r);

    let _res = sqlx::query("INSERT INTO users (name, password, email) VALUES (?, ?, ?)")
        .bind(name)
        .bind(password)
        .bind(email)
        .execute(conn)
        .await?;

    let user: UserDB = sqlx::query_as("SELECT * FROM users WHERE name = ?")
        .bind(name)
        .fetch_one(conn)
        .await?;

    let success = OnSuccessRegister {
        message: "User created succesfully".to_owned(),
        user_id: user.id,
    };

    Ok(Json(success))
}

#[allow(unused)]
pub async fn connect_to_database() -> Pool<Sqlite> {
    let options = sqlite::SqliteConnectOptions::new()
        .filename("app.db")
        .create_if_missing(true);

    let connection = sqlx::SqlitePool::connect_with(options).await.unwrap();

    // let _ = sqlx::query("PRAGMA foreign_keys = ON").execute(&connection).await;

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT UNIQUE NOT NULL,
            name TEXT NOT NULL,
            password TEXT NOT NULL
        )",
        )
        .await
        .expect("Failed to create users table");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS tokens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            token TEXT UNIQUE NOT NULL,
            user_id INTEGER NOT NULL,
            email TEXT NOT NULL,
            name TEXT NOT NULL,
            exp INTEGER NOT NULL,
            used BOOL NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
        )
        .await
        .expect("Failed to create tokens table");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    title TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
)",
        )
        .await
        .expect("Failed to create conversations table");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    token_count INTEGER,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
)",
        )
        .await
        .expect("Failed to create messages table");

    connection
}

#[derive(Serialize)]
pub struct OnSuccessTokenAdd {
    pub refresh_token: String,
}

pub async fn add_token(
    token_claims: &TokenClaims,
    token: &str,
    conn: &Pool<Sqlite>,
) -> Result<Json<OnSuccessTokenAdd>, sqlx::Error> {
    let r: Result<sqlite::SqliteQueryResult, sqlx::Error> =
        sqlx::query("INSERT INTO tokens (token, user_id, email, name, exp, used) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
            .bind(&token)
            .bind(&token_claims.user_id)
            .bind(&token_claims.email)
            .bind(&token_claims.name)
            .bind(&token_claims.exp)
            .bind(&token_claims.used)
            .execute(conn)
            .await;
    if let Err(e) = r {
        return Err(e);
    }
    Ok(Json(OnSuccessTokenAdd {
        refresh_token: token.to_string(),
    }))
}
