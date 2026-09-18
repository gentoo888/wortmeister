use axum::http::{header, HeaderValue};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

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
    /// Progress key ("<categoryId>_<setId>") of the set the user worked on most recently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_set: Option<String>,
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
    /// Where the user left off (see `resume_info`). `null` when nothing to resume.
    resume: Option<Value>,
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
    resume: Option<Value>,
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
    /// Incremented on every in-memory mutation (under `users` lock).
    version: AtomicU64,
    /// Version that was last successfully written to OneJSONFile.
    persisted_version: AtomicU64,
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

/// Write the full user map to OneJSONFile (PUT), retrying transient failures.
async fn put_users(http: &reqwest::Client, url: &str, users: &Users) -> Result<(), String> {
    const ATTEMPTS: u32 = 3;
    let mut last_err = String::new();

    for attempt in 1..=ATTEMPTS {
        let result = http.put(url).json(users).send().await;
        match result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(());
                }
                let body = resp.text().await.unwrap_or_default();
                last_err = format!("PUT {} returned HTTP {}: {}", url, status, body);
                // Client errors (4xx) other than 429 will not get better with a retry.
                if status.is_client_error() && status.as_u16() != 429 {
                    break;
                }
            }
            Err(e) => last_err = format!("PUT {} failed: {}", url, e),
        }
        if attempt < ATTEMPTS {
            eprintln!(
                "[onejson] {} (attempt {}/{}), retrying",
                last_err, attempt, ATTEMPTS
            );
            tokio::time::sleep(std::time::Duration::from_millis(300 * u64::from(attempt))).await;
        }
    }
    Err(last_err)
}

/// Snapshot the in-memory users under the lock, release the lock, then PUT the snapshot.
///
/// The `parking_lot` guard lives only inside the inner block and is dropped before the
/// first `.await`. The async `persist_lock` orders concurrent writes so the last PUT
/// always carries the newest state.
///
/// Every mutation bumps `version`; if a queued persist finds that a newer snapshot has
/// already been written (rapid answer bursts), it skips the redundant PUT.
async fn persist(state: &AppState) -> bool {
    let _serial = state.persist_lock.lock().await;

    let (snapshot, version): (Users, u64) = {
        let users = state.users.lock();
        (users.clone(), state.version.load(Ordering::Acquire))
    };

    if version <= state.persisted_version.load(Ordering::Acquire) {
        return true; // already persisted by an earlier, coalesced write
    }

    match put_users(&state.http, &state.onejson_url, &snapshot).await {
        Ok(()) => {
            state.persisted_version.fetch_max(version, Ordering::AcqRel);
            true
        }
        Err(e) => {
            eprintln!("[onejson] persist error: {}", e);
            false
        }
    }
}

/// Call while holding the `users` lock, right after mutating the map.
fn mark_dirty(state: &AppState) {
    state.version.fetch_add(1, Ordering::AcqRel);
}

/// Split a progress key "<categoryId>_<setId>" into its parts (category ids may
/// themselves contain underscores, so split on the last one).
fn split_progress_key(key: &str) -> Option<(&str, &str)> {
    let idx = key.rfind('_')?;
    if idx == 0 || idx + 1 >= key.len() {
        return None;
    }
    Some((&key[..idx], &key[idx + 1..]))
}

fn entry_updated_at(entry: &Value) -> &str {
    entry.get("updatedAt").and_then(Value::as_str).unwrap_or("")
}

/// Pick the progress key of the most recently updated entry (ISO-8601 strings sort
/// lexicographically). Returns `None` when there is no usable entry.
fn latest_progress_key(progress: &Value) -> Option<String> {
    let map = progress.as_object()?;
    map.iter()
        .filter(|(_, v)| v.is_object())
        .max_by(|(ka, a), (kb, b)| {
            entry_updated_at(a)
                .cmp(entry_updated_at(b))
                .then_with(|| ka.cmp(kb))
        })
        .map(|(k, _)| k.clone())
}

/// Build the "resume" payload for a user: which set they were last working on,
/// how far they got, and the ids the frontend needs to reopen it. Word levels
/// themselves live in `progress[key].words` and are restored by the frontend.
fn resume_info(record: &UserRecord) -> Option<Value> {
    let map = record.progress.as_object()?;

    let key = record
        .last_set
        .as_ref()
        .filter(|k| map.get(*k).map_or(false, Value::is_object))
        .cloned()
        .or_else(|| latest_progress_key(&record.progress))?;
    let entry = map.get(&key)?;

    let (category_id, set_id) = match (
        entry.get("categoryId").and_then(Value::as_str),
        entry.get("setId"),
    ) {
        (Some(c), Some(sv)) if !c.is_empty() => {
            let sid = match sv {
                Value::String(x) => x.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            (c.to_string(), sid)
        }
        _ => {
            let (c, sid) = split_progress_key(&key)?;
            (c.to_string(), sid.to_string())
        }
    };

    let words = entry.get("words").and_then(Value::as_array);
    let total = words.map_or(0, |w| w.len());
    let mastered = words.map_or(0, |w| {
        w.iter()
            .filter(|x| x.get("level").and_then(Value::as_u64).unwrap_or(0) >= 5)
            .count()
    });

    Some(serde_json::json!({
        "key": key,
        "categoryId": category_id,
        "setId": set_id,
        "setName": entry.get("setName").cloned().unwrap_or(Value::Null),
        "category": entry.get("category").cloned().unwrap_or(Value::Null),
        "masteredCount": mastered,
        "totalCount": total,
        "finished": total > 0 && mastered >= total,
        "updatedAt": entry_updated_at(entry),
    }))
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
                resume: None,
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
                    resume: None,
                }),
            );
        }

        let record = UserRecord {
            username: username.clone(),
            password_hash: hash_password(&key, &req.password),
            token: Some(token.clone()),
            progress: Value::Object(Default::default()),
            stats: Value::Object(Default::default()),
            last_set: None,
        };
        users.insert(key, record.clone());
        mark_dirty(&state);
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
            resume: None,
        }),
    )
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthRequest>,
) -> (StatusCode, Json<AuthResponse>) {
    let username = req.username.trim().to_lowercase();
    let token = uuid::Uuid::new_v4().to_string();

    let (progress, stats, resume) = {
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
                    resume: None,
                }),
            );
        }

        let record = users.get_mut(&username).unwrap();
        record.token = Some(token.clone());
        mark_dirty(&state);
        (
            record.progress.clone(),
            record.stats.clone(),
            resume_info(record),
        )
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
            resume,
        }),
    )
}

async fn save_progress(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> (StatusCode, Json<SaveResponse>) {
    let username = req.username.trim().to_lowercase();

    // Progress/stats must be JSON objects. Reject anything else instead of overwriting
    // the stored record with garbage (or an empty map).
    if !req.progress.is_object() || !req.stats.is_object() {
        return (
            StatusCode::BAD_REQUEST,
            Json(SaveResponse {
                success: false,
                message: "Geçersiz ilerleme verisi.".to_string(),
            }),
        );
    }

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
                // Remember which set the user is working on: prefer the entry that
                // actually changed in this save, otherwise the most recently updated one.
                let changed_key = req.progress.as_object().and_then(|new_map| {
                    let old_map = record.progress.as_object();
                    let mut changed: Vec<(&String, &Value)> = new_map
                        .iter()
                        .filter(|(k, v)| old_map.and_then(|o| o.get(*k)) != Some(*v))
                        .collect();
                    changed.sort_by(|(_, a), (_, b)| entry_updated_at(a).cmp(entry_updated_at(b)));
                    changed.last().map(|(k, _)| (*k).clone())
                });
                record.progress = req.progress;
                record.stats = req.stats;
                record.last_set = changed_key
                    .or_else(|| record.last_set.clone())
                    .or_else(|| latest_progress_key(&record.progress));
                mark_dirty(&state);
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

    if persist(&state).await {
        (
            StatusCode::OK,
            Json(SaveResponse {
                success: true,
                message: "İlerleme kaydedildi.".to_string(),
            }),
        )
    } else {
        // In-memory state is updated; remote write failed. Tell the client so it keeps
        // its local copy, but do NOT invalidate the session.
        (
            StatusCode::OK,
            Json(SaveResponse {
                success: true,
                message: "İlerleme bellekte kaydedildi, uzak depoya yazılamadı.".to_string(),
            }),
        )
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
                resume: None,
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
                resume: resume_info(record),
            }),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(LoadResponse {
                success: false,
                message: "Kullanıcı bulunamadı.".to_string(),
                progress: None,
                stats: None,
                resume: None,
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
        version: AtomicU64::new(0),
        persisted_version: AtomicU64::new(0),
    });

    let cors = CorsLayer::permissive();

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../static".to_string());

    let app = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/save", post(save_progress))
        .route("/api/auth/load", post(load_progress))
        .fallback_service(ServeDir::new(static_dir))
        // Frontend files are small; force revalidation so a redeploy never leaves
        // browsers running a stale app.js against a newer API.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .layer(cors)
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("GIKAL Wortmeister (WASM) running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
