use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::AppState;

// Request/Response types
#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct NoteResponse {
    pub id: String,
    pub content: String,
    pub updated_at: String,
}

pub struct AuthInfo {
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct UpdateNoteRequest {
    pub content: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// Handlers
pub fn signup(state: &Arc<AppState>, body: &str) -> Result<String, (u16, String)> {
    let req: SignupRequest =
        serde_json::from_str(body).map_err(|_| (400, json_error("Invalid request body")))?;

    // Validate
    if req.email.is_empty() || !req.email.contains('@') {
        return Err((400, json_error("Invalid email")));
    }
    if req.password.len() < 8 {
        return Err((400, json_error("Password must be at least 8 characters")));
    }

    // Check if user exists
    if state
        .db
        .get_user_by_email(&req.email)
        .map_err(db_error)?
        .is_some()
    {
        return Err((409, json_error("Email already registered")));
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(req.password.as_bytes(), &salt)
        .map_err(|_| (500, json_error("Failed to hash password")))?
        .to_string();

    // Create user
    let user_id = ulid::Ulid::new().to_string();
    state
        .db
        .create_user(&user_id, &req.email, &password_hash)
        .map_err(db_error)?;

    // Create session
    let token = generate_token();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    state
        .db
        .create_session(&token, &user_id, &expires_at)
        .map_err(db_error)?;

    Ok(serde_json::to_string(&AuthResponse { token }).unwrap())
}

pub fn login(state: &Arc<AppState>, body: &str) -> Result<String, (u16, String)> {
    let req: LoginRequest =
        serde_json::from_str(body).map_err(|_| (400, json_error("Invalid request body")))?;

    // Get user
    let user = state
        .db
        .get_user_by_email(&req.email)
        .map_err(db_error)?
        .ok_or_else(|| (404, json_error("User not found")))?;

    // Verify password
    let parsed_hash =
        PasswordHash::new(&user.password_hash).map_err(|_| (500, json_error("Internal error")))?;

    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| (401, json_error("Invalid credentials")))?;

    // Create session
    let token = generate_token();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();
    state
        .db
        .create_session(&token, &user.id, &expires_at)
        .map_err(db_error)?;

    Ok(serde_json::to_string(&AuthResponse { token }).unwrap())
}

pub fn logout(state: &Arc<AppState>, token: &str) -> Result<String, (u16, String)> {
    state.db.delete_session(token).map_err(db_error)?;
    Ok("{}".to_string())
}

pub fn get_note(state: &Arc<AppState>, user_id: &str) -> Result<String, (u16, String)> {
    let note = state.db.get_or_create_note(user_id).map_err(db_error)?;

    Ok(serde_json::to_string(&NoteResponse {
        id: note.id,
        content: note.content,
        updated_at: note.updated_at,
    })
    .unwrap())
}

pub fn update_note(
    state: &Arc<AppState>,
    user_id: &str,
    body: &str,
) -> Result<String, (u16, String)> {
    let req: UpdateNoteRequest =
        serde_json::from_str(body).map_err(|_| (400, json_error("Invalid request body")))?;

    let note = state
        .db
        .update_note(user_id, &req.content)
        .map_err(db_error)?;

    Ok(serde_json::to_string(&NoteResponse {
        id: note.id,
        content: note.content,
        updated_at: note.updated_at,
    })
    .unwrap())
}

// Auth middleware
pub fn authenticate(
    state: &Arc<AppState>,
    auth_header: Option<&str>,
) -> Result<AuthInfo, (u16, String)> {
    let token = auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| (401, json_error("Missing authorization")))?;

    let session = state
        .db
        .get_session(token)
        .map_err(db_error)?
        .ok_or_else(|| (401, json_error("Invalid token")))?;

    // Check expiration
    let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .map_err(|_| (500, json_error("Internal error")))?;

    if expires_at < chrono::Utc::now() {
        state.db.delete_session(token).ok();
        return Err((401, json_error("Token expired")));
    }

    Ok(AuthInfo {
        user_id: session.user_id,
    })
}

// Helpers
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn json_error(msg: &str) -> String {
    serde_json::to_string(&ErrorResponse {
        error: msg.to_string(),
    })
    .unwrap()
}

fn db_error(err: rusqlite::Error) -> (u16, String) {
    eprintln!("Database error: {:?}", err);
    (500, json_error("Database error"))
}
