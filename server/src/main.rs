use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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

type Users = HashMap<String, UserRecord>;

struct AppState {
    /// OneJSONFile file endpoint (from `ONEJSON_URL`).
    onejson_url: String,
    http: reqwest::Client,
    /// In-memory copy of all users. Never hold this guard across an `.await`.
    users: Mutex<Users>,
    /// Serialises remote writes so an older snapshot can never overwrite a newer one.
    persist_lock: tokio::sync::Mutex<()>,
}

fn hash_password(username: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

/// Load the full user map from OneJSONFile (GET).
///
/// An empty body, `null`, or an empty object is treated as "no users yet".
async fn fetch_users(http: &reqwest::Client, url: &str) -> Result<Users, String> {
    let resp = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("GET {} failed: {}", url, e))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("GET {} body read failed: {}", url, e))?;

    if !status.is_success() {
        return Err(format!("GET {} returned HTTP {}: {}", url, status, body));
    }

    if body.trim().is_empty() {
        return Ok(Users::new());
    }

    let value: Value = serde_json::from_str(&body)
        .map_err(|e| format!("GET {} returned invalid JSON: {}", url, e))?;

    match value {
        Value::Null => Ok(Users::new()),
        Value::Object(_) => serde_json::from_value(value)
            .map_err(|e| format!("GET {} JSON is not a user map: {}", url, e)),
        other => Err(format!(
            "GET {} returned unexpected JSON type: {}",
            url,
            match other {
                Value::Array(_) => "array",
                Value::String(_) => "string",
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                _ => "unknown",
            }
        )),
    }
}

/// Write the full user map to OneJSONFile (PUT).
async fn put_users(http: &reqwest::Client, url: &str, users: &Users) -> Result<(), String> {
    let resp = http
        .put(url)
        .json(users)
        .send()
        .await
        .map_err(|e| format!("PUT {} failed: {}", url, e))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("PUT {} returned HTTP {}: {}", url, status, body));
    }
    Ok(())
}

/// Snapshot the in-memory users under the lock, release the lock, then PUT the snapshot.
///
/// The `parking_lot` guard lives only inside the inner block and is dropped before the
/// first `.await`. The async `persist_lock` orders concurrent writes so the last PUT
/// always carries the newest state.
async fn persist(state: &AppState) {
    let _serial = state.persist_lock.lock().await;

    let snapshot: Users = {
        let users = state.users.lock();
        users.clone()
    };

    if let Err(e) = put_users(&state.http, &state.onejson_url, &snapshot).await {
        eprintln!("[onejson] persist error: {}", e);
    }
}

fn valid_token(users: &Users, username: &str, token: &str) -> bool {
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

    let token = uuid::Uuid::new_v4().to_string();
    let key = username.to_lowercase();

    // Mutate under the lock; the guard is dropped at the end of this block.
    let record = {
        let mut users = state.users.lock();
        if users.contains_key(&key) {
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
            password_hash: hash_password(&key, &req.password),
            token: Some(token.clone()),
            progress: Value::Object(Default::default()),
            stats: Value::Object(Default::default()),
        };
        users.insert(key, record.clone());
        record
    };

    persist(&state).await;

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
    let token = uuid::Uuid::new_v4().to_string();

    let (progress, stats) = {
        let mut users = state.users.lock();

        let valid = users.get(&username).map_or(false, |r| {
            r.password_hash == hash_password(&username, &req.password)
        });

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

        let record = users.get_mut(&username).unwrap();
        record.token = Some(token.clone());
        (record.progress.clone(), record.stats.clone())
    };

    persist(&state).await;

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

    {
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
            }
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(SaveResponse {
                        success: false,
                        message: "Kullanıcı bulunamadı.".to_string(),
                    }),
                );
            }
        }
    }

    persist(&state).await;

    (
        StatusCode::OK,
        Json(SaveResponse {
            success: true,
            message: "İlerleme kaydedildi.".to_string(),
        }),
    )
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
    let onejson_url = match std::env::var("ONEJSON_URL") {
        Ok(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => {
            eprintln!(
                "ONEJSON_URL is not set. Example:\n  \
                 ONEJSON_URL=https://onejsonfile.com/api/v1/files/<file-id>"
            );
            std::process::exit(1);
        }
    };

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("failed to build HTTP client");

    // Refuse to start with an empty map if the remote can't be read; otherwise the first
    // PUT would overwrite the remote data.
    let users = match fetch_users(&http, &onejson_url).await {
        Ok(users) => users,
        Err(e) => {
            eprintln!("[onejson] failed to load users: {}", e);
            std::process::exit(1);
        }
    };
    println!("Loaded {} users from OneJSONFile", users.len());

    let state = Arc::new(AppState {
        onejson_url,
        http,
        users: Mutex::new(users),
        persist_lock: tokio::sync::Mutex::new(()),
    });

    let cors = CorsLayer::permissive();

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../static".to_string());

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
