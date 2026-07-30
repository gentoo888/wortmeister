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
    #[serde(default)]
    token: Option<String>,
    #[serde(default = "empty_object")]
    progress: Value,
    #[serde(default = "empty_object")]
    stats: Value,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
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

#[derive(Debug, Deserialize)]
struct LoadRequest {
    username: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct SaveResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct LoadResponse {
    success: bool,
    message: String,
    progress: Option<Value>,
    stats: Option<Value>,
}

struct AppState {
    users_file: PathBuf,
    users: Mutex<HashMap<String, UserRecord>>,
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

fn valid_token(users: &HashMap<String, UserRecord>, username: &str, token: &str) -> bool {
    users
        .get(username)
        .and_then(|r| r.token.as_deref())
        .map_or(false, |t| !token.is_empty() && t == token)
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

    let token = uuid::Uuid::new_v4().to_string();
    let record = UserRecord {
        username: username.clone(),
        password_hash: hash_password(&username.to_lowercase(), &req.password),
        token: Some(token.clone()),
        progress: Value::Object(Default::default()),
        stats: Value::Object(Default::default()),
    };
    users.insert(username.to_lowercase(), record.clone());
    save_users(&state.users_file, &users);
    drop(users);

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
    let mut users = state.users.lock();

    let valid = users
        .get(&username)
        .map_or(false, |r| r.password_hash == hash_password(&username, &req.password));

    if !valid {
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
    let record = users.get_mut(&username).unwrap();
    record.token = Some(token.clone());
    let progress = record.progress.clone();
    let stats = record.stats.clone();
    save_users(&state.users_file, &users);

    (
        StatusCode::OK,
        Json(AuthResponse {
            success: true,
            message: "Giriş başarılı.".to_string(),
            token: Some(token),
            progress: Some(progress),
            stats: Some(stats),
        }),
    )
}

async fn save_progress(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> (StatusCode, Json<SaveResponse>) {
    let username = req.username.trim().to_lowercase();
    let mut users = state.users.lock();

    if !valid_token(&users, &username, &req.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(SaveResponse {
                success: false,
                message: "Oturum geçersiz. Lütfen tekrar giriş yapın.".to_string(),
            }),
        );
    }

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

async fn load_progress(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoadRequest>,
) -> (StatusCode, Json<LoadResponse>) {
    let username = req.username.trim().to_lowercase();
    let users = state.users.lock();

    if !valid_token(&users, &username, &req.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(LoadResponse {
                success: false,
                message: "Oturum geçersiz. Lütfen tekrar giriş yapın.".to_string(),
                progress: None,
                stats: None,
            }),
        );
    }

    match users.get(&username) {
        Some(record) => (
            StatusCode::OK,
            Json(LoadResponse {
                success: true,
                message: "İlerleme yüklendi.".to_string(),
                progress: Some(record.progress.clone()),
                stats: Some(record.stats.clone()),
            }),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(LoadResponse {
                success: false,
                message: "Kullanıcı bulunamadı.".to_string(),
                progress: None,
                stats: None,
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
    });

    let cors = CorsLayer::permissive();

    let static_dir =
        std::env::var("STATIC_DIR").unwrap_or_else(|_| "../static".to_string());

    let app = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/save", post(save_progress))
        .route("/api/auth/load", post(load_progress))
        .fallback_service(ServeDir::new(static_dir))
        .layer(cors)
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("GIKAL Wortmeister (WASM) running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
