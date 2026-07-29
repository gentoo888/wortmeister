use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UserRecord {
    username: String,
    password_hash: String,
    progress: Value,
    stats: Value,
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    success: bool,
    message: String,
    token: Option<String>,
    progress: Option<Value>,
    stats: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SaveRequest {
    username: String,
    token: String,
    progress: Value,
    stats: Value,
}

#[derive(Debug, Serialize)]
struct SaveResponse {
    success: bool,
    message: String,
}

struct AppState {
    users_file: PathBuf,
    users: Mutex<HashMap<String, UserRecord>>,
    tokens: Mutex<HashMap<String, String>>,
}

fn hash_password(username: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn load_users(path: &PathBuf) -> HashMap<String, UserRecord> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_users(path: &PathBuf, users: &HashMap<String, UserRecord>) {
    if let Ok(content) = serde_json::to_string_pretty(users) {
        let _ = std::fs::write(path, content);
    }
}

fn valid_token(state: &AppState, username: &str, token: &str) -> bool {
    let tokens = state.tokens.lock();
    tokens.get(token).map_or(false, |u| u == username)
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> (StatusCode, Json<AuthResponse>) {
    let username = req.username.trim().to_string();
    if username.len() < 3 || req.password.len() < 4 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Kullanıcı adı en az 3, şifre en az 4 karakter olmalı.".to_string(),
                token: None,
                progress: None,
                stats: None,
            }),
        );
    }

    let mut users = state.users.lock();
    if users.contains_key(&username.to_lowercase()) {
        return (
            StatusCode::CONFLICT,
            Json(AuthResponse {
                success: false,
                message: "Bu kullanıcı adı zaten alınmış.".to_string(),
                token: None,
                progress: None,
                stats: None,
            }),
        );
    }

    let record = UserRecord {
        username: username.clone(),
        password_hash: hash_password(&username.to_lowercase(), &req.password),
        progress: Value::Object(Default::default()),
        stats: Value::Object(Default::default()),
    };
    users.insert(username.to_lowercase(), record.clone());
    save_users(&state.users_file, &users);
    drop(users);

    let token = uuid::Uuid::new_v4().to_string();
    state
        .tokens
        .lock()
        .insert(token.clone(), username.to_lowercase());

    (
        StatusCode::OK,
        Json(AuthResponse {
            success: true,
            message: "Kayıt başarılı.".to_string(),
            token: Some(token),
            progress: Some(record.progress),
            stats: Some(record.stats),
        }),
    )
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> (StatusCode, Json<AuthResponse>) {
    let username = req.username.trim().to_lowercase();
    let users = state.users.lock();

    let record = match users.get(&username) {
        Some(r) => r.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthResponse {
                    success: false,
                    message: "Kullanıcı adı veya şifre hatalı.".to_string(),
                    token: None,
                    progress: None,
                    stats: None,
                }),
            );
        }
    };
    drop(users);

    if record.password_hash != hash_password(&username, &req.password) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthResponse {
                success: false,
                message: "Kullanıcı adı veya şifre hatalı.".to_string(),
                token: None,
                progress: None,
                stats: None,
            }),
        );
    }

    let token = uuid::Uuid::new_v4().to_string();
    state.tokens.lock().insert(token.clone(), username);

    (
        StatusCode::OK,
        Json(AuthResponse {
            success: true,
            message: "Giriş başarılı.".to_string(),
            token: Some(token),
            progress: Some(record.progress),
            stats: Some(record.stats),
        }),
    )
}

async fn save_progress(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> (StatusCode, Json<SaveResponse>) {
    let username = req.username.trim().to_lowercase();

    if !valid_token(&state, &username, &req.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(SaveResponse {
                success: false,
                message: "Oturum geçersiz. Lütfen tekrar giriş yapın.".to_string(),
            }),
        );
    }

    let mut users = state.users.lock();
    match users.get_mut(&username) {
        Some(record) => {
            record.progress = req.progress;
            record.stats = req.stats;
            save_users(&state.users_file, &users);
            (
                StatusCode::OK,
                Json(SaveResponse {
                    success: true,
                    message: "İlerleme kaydedildi.".to_string(),
                }),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(SaveResponse {
                success: false,
                message: "Kullanıcı bulunamadı.".to_string(),
            }),
        ),
    }
}

#[tokio::main]
async fn main() {
    let users_file = PathBuf::from(
        std::env::var("USERS_FILE").unwrap_or_else(|_| "users.json".to_string()),
    );
    let users = load_users(&users_file);
    println!("Loaded {} users from {:?}", users.len(), users_file);

    let state = Arc::new(AppState {
        users_file,
        users: Mutex::new(users),
        tokens: Mutex::new(HashMap::new()),
    });

    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/save", post(save_progress))
        .fallback_service(ServeDir::new("static"))
        .layer(cors)
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("GIKAL Wortmeister (WASM) running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
