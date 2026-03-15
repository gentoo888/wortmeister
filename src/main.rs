use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use strsim::normalized_levenshtein;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Word {
    foreign: String,
    translation: String,
    level: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CategoryInfo {
    id: String,
    name: String,
    set_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SetInfo {
    id: String,
    name: String,
    word_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CheckRequest {
    session_id: String,
    word_index: usize,
    answer: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CheckResponse {
    correct: bool,
    similarity: f64,
    close_match: bool,
    correct_answer: String,
    old_level: u8,
    new_level: u8,
    feedback: String,
    all_mastered: bool,
    mastered_count: usize,
    total_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionState {
    words: Vec<Word>,
    category: String,
    set_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StartGameRequest {
    category_id: String,
    set_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StartGameResponse {
    session_id: String,
    words: Vec<Word>,
    category: String,
    set_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AddWordRequest {
    session_id: String,
    foreign: String,
    translation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResetRequest {
    session_id: String,
}

struct AppState {
    catalog: HashMap<String, HashMap<String, Vec<Word>>>,
    category_names: HashMap<String, String>,
    sessions: RwLock<HashMap<String, SessionState>>,
}

fn normalize_text(s: &str) -> String {
    let s = s.nfkd().collect::<String>();
    let s = s.trim().to_lowercase();
    s.replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss")
        .replace('ş', "s")
        .replace('ç', "c")
        .replace('ğ', "g")
        .replace('ı', "i")
        .replace('İ', "i")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_article(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let prefixes = [
        "der ", "die ", "das ", "den ", "dem ", "des ", "ein ", "eine ", "einen ", "einem ",
        "einer ", "eines ",
    ];
    for prefix in &prefixes {
        if s.starts_with(prefix) {
            return s[prefix.len()..].to_string();
        }
    }
    s
}

fn check_word(user_answer: &str, correct: &str) -> (bool, f64, bool) {
    let user_raw = user_answer.trim().to_lowercase();
    let correct_raw = correct.trim().to_lowercase();

    if user_raw == correct_raw {
        return (true, 1.0, false);
    }

    let user_nospace: String = user_raw.chars().filter(|c| !c.is_whitespace()).collect();
    let correct_nospace: String = correct_raw.chars().filter(|c| !c.is_whitespace()).collect();
    if user_nospace == correct_nospace {
        return (true, 1.0, false);
    }

    let user_norm = normalize_text(&user_raw);
    let correct_norm = normalize_text(&correct_raw);
    if user_norm == correct_norm {
        return (true, 0.95, false);
    }

    let user_no_art = strip_article(&user_raw);
    let correct_no_art = strip_article(&correct_raw);
    if user_no_art == correct_no_art
        || normalize_text(&user_no_art) == normalize_text(&correct_no_art)
    {
        return (true, 0.90, false);
    }

    let alternatives: Vec<&str> = correct_raw.split('/').collect();
    if alternatives.len() > 1 {
        for alt in &alternatives {
            let alt = alt.trim();
            if user_raw == alt || normalize_text(&user_raw) == normalize_text(alt) {
                return (true, 0.95, false);
            }
            let sim = normalized_levenshtein(&normalize_text(&user_raw), &normalize_text(alt));
            if sim >= 0.85 {
                return (true, sim, true);
            }
        }
    }

    let base = correct_raw.split('(').next().unwrap_or(&correct_raw).trim();
    if !base.is_empty() && (user_raw == base || normalize_text(&user_raw) == normalize_text(base)) {
        return (true, 0.90, false);
    }

    let similarity = normalized_levenshtein(&user_norm, &correct_norm);
    if similarity >= 0.85 {
        return (true, similarity, true);
    }

    let close = similarity >= 0.65;
    (false, similarity, close)
}

fn load_catalog() -> (
    HashMap<String, HashMap<String, Vec<Word>>>,
    HashMap<String, String>,
) {
    let mut catalog: HashMap<String, HashMap<String, Vec<Word>>> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();

    let load_dir = |dir: &str,
                    cat_id: &str,
                    cat_name: &str,
                    catalog: &mut HashMap<String, HashMap<String, Vec<Word>>>,
                    names: &mut HashMap<String, String>| {
        names.insert(cat_id.to_string(), cat_name.to_string());
        let cat_map = catalog.entry(cat_id.to_string()).or_default();

        // 🔧 CARGO_MANIFEST_DIR kullan - bu Cargo.toml'ün olduğu klasör
        let words_base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("words")
            .join(dir);

        println!("Loading words from: {:?}", words_base);

        if let Ok(entries) = std::fs::read_dir(&words_base) {
            let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            files.sort_by_key(|e| e.file_name());

            for (idx, entry) in files.iter().enumerate() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        match serde_json::from_str::<Vec<Word>>(&content) {
                            Ok(words) => {
                                let set_id = (idx + 1).to_string();
                                cat_map.insert(set_id, words);
                            }
                            Err(e) => {
                                eprintln!("JSON parse error in {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!("Could not read directory: {:?}", words_base);
        }
    };

    load_dir("hazirlik", "hazirlik", "Hazırlık", &mut catalog, &mut names);
    load_dir(
        "hazirlik2_donem",
        "hazirlik2_donem",
        "Hazırlık 2. Dönem",
        &mut catalog,
        &mut names,
    );
    load_dir(
        "9_10_sinif",
        "sinif_9_10",
        "9-10. Sınıf",
        &mut catalog,
        &mut names,
    );

    (catalog, names)
}

async fn get_categories(State(state): State<Arc<AppState>>) -> Json<Vec<CategoryInfo>> {
    let mut cats: Vec<CategoryInfo> = state
        .catalog
        .iter()
        .map(|(id, sets)| CategoryInfo {
            id: id.clone(),
            name: state
                .category_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.clone()),
            set_count: sets.len(),
        })
        .collect();
    cats.sort_by(|a, b| a.name.cmp(&b.name));
    Json(cats)
}

async fn get_sets(
    State(state): State<Arc<AppState>>,
    Path(category_id): Path<String>,
) -> Result<Json<Vec<SetInfo>>, StatusCode> {
    let sets = state
        .catalog
        .get(&category_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut result: Vec<SetInfo> = sets
        .iter()
        .map(|(id, words)| SetInfo {
            id: id.clone(),
            name: format!("{}. Ünite", id),
            word_count: words.len(),
        })
        .collect();
    result.sort_by_key(|s| s.id.parse::<u32>().unwrap_or(0));
    Ok(Json(result))
}

async fn start_game(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartGameRequest>,
) -> Result<Json<StartGameResponse>, StatusCode> {
    let sets = state
        .catalog
        .get(&req.category_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let words = sets.get(&req.set_id).ok_or(StatusCode::NOT_FOUND)?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let cat_name = state
        .category_names
        .get(&req.category_id)
        .cloned()
        .unwrap_or_else(|| req.category_id.clone());
    let set_name = format!("{} / {}. Ünite", cat_name, req.set_id);

    let session = SessionState {
        words: words.clone(),
        category: cat_name,
        set_name: set_name.clone(),
    };

    state.sessions.write().insert(session_id.clone(), session);

    Ok(Json(StartGameResponse {
        session_id,
        words: words.clone(),
        category: state
            .category_names
            .get(&req.category_id)
            .cloned()
            .unwrap_or_default(),
        set_name,
    }))
}

async fn check_answer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, StatusCode> {
    let mut sessions = state.sessions.write();
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    if req.word_index >= session.words.len() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let word = &session.words[req.word_index];
    let old_level = word.level;
    let correct_answer = word.translation.clone();
    let foreign = word.foreign.clone();

    let (is_correct, similarity, close_match) = check_word(&req.answer, &correct_answer);

    let new_level;
    let feedback;

    if is_correct {
        let w = &mut session.words[req.word_index];
        if w.level < 5 {
            w.level += 1;
        }
        new_level = w.level;

        if similarity >= 0.99 {
            feedback = format!("✅ Mükemmel! Seviye: {} → {}", old_level, new_level);
        } else if close_match {
            feedback = format!(
                "✅ Doğru! (Tam yazılışı: \"{}\") Seviye: {} → {}",
                correct_answer, old_level, new_level
            );
        } else {
            feedback = format!("✅ Doğru! Seviye: {} → {}", old_level, new_level);
        }
    } else {
        let w = &mut session.words[req.word_index];
        if w.level > 1 {
            w.level -= 1;
        }
        new_level = w.level;

        if close_match {
            feedback = format!(
                "❌ Neredeyse! Doğru cevap: \"{}\" ({}). Seviye: {} → {}",
                correct_answer, foreign, old_level, new_level
            );
        } else {
            feedback = format!(
                "❌ Yanlış! Doğru cevap: \"{}\". Seviye: {} → {}",
                correct_answer, old_level, new_level
            );
        }
    }

    let mastered_count = session.words.iter().filter(|w| w.level >= 5).count();
    let total_count = session.words.len();
    let all_mastered = mastered_count == total_count && total_count > 0;

    Ok(Json(CheckResponse {
        correct: is_correct,
        similarity,
        close_match,
        correct_answer,
        old_level,
        new_level,
        feedback,
        all_mastered,
        mastered_count,
        total_count,
    }))
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionState>, StatusCode> {
    let sessions = state.sessions.read();
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(session.clone()))
}

async fn reset_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetRequest>,
) -> Result<Json<SessionState>, StatusCode> {
    let mut sessions = state.sessions.write();
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    for w in &mut session.words {
        w.level = 1;
    }
    Ok(Json(session.clone()))
}

async fn add_word(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddWordRequest>,
) -> Result<Json<SessionState>, StatusCode> {
    let mut sessions = state.sessions.write();
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    session.words.push(Word {
        foreign: req.foreign,
        translation: req.translation,
        level: 1,
    });
    Ok(Json(session.clone()))
}

async fn create_custom_session() -> Json<StartGameResponse> {
    let session_id = uuid::Uuid::new_v4().to_string();
    Json(StartGameResponse {
        session_id,
        words: vec![],
        category: "Özel".to_string(),
        set_name: "Özel Kelime Listesi".to_string(),
    })
}

#[tokio::main]
async fn main() {
    let (catalog, category_names) = load_catalog();

    println!("📚 Loaded catalog:");
    for (cat_id, sets) in &catalog {
        let name = category_names.get(cat_id).unwrap_or(cat_id);
        let total_words: usize = sets.values().map(|v| v.len()).sum();
        println!(
            "  {} ({}): {} sets, {} words",
            name,
            cat_id,
            sets.len(),
            total_words
        );
    }

    let state = Arc::new(AppState {
        catalog,
        category_names,
        sessions: RwLock::new(HashMap::new()),
    });

    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/api/categories", get(get_categories))
        .route("/api/categories/:category_id/sets", get(get_sets))
        .route("/api/game/start", post(start_game))
        .route("/api/game/check", post(check_answer))
        .route("/api/game/session/:session_id", get(get_session))
        .route("/api/game/reset", post(reset_session))
        .route("/api/game/add-word", post(add_word))
        .route("/api/game/custom-session", post(create_custom_session))
        .fallback_service(ServeDir::new("static"))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:9090";
    println!("🚀 GİKAL Wortmeister running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
