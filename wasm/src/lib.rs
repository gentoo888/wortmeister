use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use strsim::normalized_levenshtein;
use unicode_normalization::UnicodeNormalization;
use wasm_bindgen::prelude::*;

type Catalog = (
    HashMap<String, HashMap<String, Vec<Word>>>,
    HashMap<String, String>,
);

static CATALOG: OnceLock<Catalog> = OnceLock::new();

fn catalog_ref() -> &'static Catalog {
    CATALOG.get_or_init(load_catalog)
}

static WORDS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../words");

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Word {
    foreign: String,
    translation: String,
    level: u8,
}

#[derive(Clone, Debug, Serialize)]
struct CategoryInfo {
    id: String,
    name: String,
    set_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct SetInfo {
    id: String,
    name: String,
    word_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct CheckResult {
    correct: bool,
    similarity: f64,
    close_match: bool,
    correct_answer: String,
    old_level: u8,
    new_level: u8,
    feedback: String,
}

fn category_defs() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("hazirlik", "hazirlik", "Hazırlık"),
        ("hazirlik2_donem", "hazirlik2_donem", "Hazırlık 2. Dönem"),
        ("9_10_sinif", "sinif_9_10", "9-10. Sınıf"),
    ]
}

fn load_catalog() -> (
    HashMap<String, HashMap<String, Vec<Word>>>,
    HashMap<String, String>,
) {
    let mut catalog: HashMap<String, HashMap<String, Vec<Word>>> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();

    for (dir_name, cat_id, cat_name) in category_defs() {
        names.insert(cat_id.to_string(), cat_name.to_string());
        let cat_map = catalog.entry(cat_id.to_string()).or_default();

        if let Some(sub_dir) = WORDS_DIR.get_dir(dir_name) {
            for file in sub_dir.files() {
                let path = file.path();
                if !path.extension().map_or(false, |e| e == "json") {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let num: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
                let set_id = if num.is_empty() { stem.to_string() } else { num };
                if let Some(content) = file.contents_utf8() {
                    if let Ok(words) = serde_json::from_str::<Vec<Word>>(content) {
                        cat_map.insert(set_id, words);
                    }
                }
            }
        }
    }

    (catalog, names)
}

fn normalize_text(s: &str) -> String {
    let s = s.nfkd().collect::<String>();
    let s = s.trim().to_lowercase();
    s.replace('\u{00e4}', "ae")
        .replace('\u{00f6}', "oe")
        .replace('\u{00fc}', "ue")
        .replace('\u{00df}', "ss")
        .replace('\u{015f}', "s")
        .replace('\u{00e7}', "c")
        .replace('\u{011f}', "g")
        .replace('\u{0131}', "i")
        .replace('\u{0130}', "i")
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

#[wasm_bindgen]
pub fn get_categories() -> String {
    let (catalog, names) = catalog_ref();
    let mut cats: Vec<CategoryInfo> = catalog
        .iter()
        .map(|(id, sets)| CategoryInfo {
            id: id.clone(),
            name: names.get(id).cloned().unwrap_or_else(|| id.clone()),
            set_count: sets.len(),
        })
        .collect();
    cats.sort_by(|a, b| a.name.cmp(&b.name));
    serde_json::to_string(&cats).unwrap_or_else(|_| "[]".to_string())
}

#[wasm_bindgen]
pub fn get_sets(category_id: &str) -> String {
    let (catalog, _) = catalog_ref();
    match catalog.get(category_id) {
        Some(sets) => {
            let mut result: Vec<SetInfo> = sets
                .iter()
                .map(|(id, words)| SetInfo {
                    id: id.clone(),
                    name: format!("{}. Ünite", id),
                    word_count: words.len(),
                })
                .collect();
            result.sort_by_key(|s| s.id.parse::<u32>().unwrap_or(0));
            serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
        }
        None => "[]".to_string(),
    }
}

#[wasm_bindgen]
pub fn get_words(category_id: &str, set_id: &str) -> String {
    let (catalog, _) = catalog_ref();
    catalog
        .get(category_id)
        .and_then(|sets| sets.get(set_id))
        .map(|words| serde_json::to_string(words).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string())
}

#[wasm_bindgen]
pub fn get_category_name(category_id: &str) -> String {
    let (_, names) = catalog_ref();
    names
        .get(category_id)
        .cloned()
        .unwrap_or_else(|| category_id.to_string())
}

#[wasm_bindgen]
pub fn check_answer(user_answer: &str, correct_answer: &str, foreign: &str, old_level: u8) -> String {
    let (is_correct, similarity, close_match) = check_word(user_answer, correct_answer);

    let new_level;
    let feedback;

    if is_correct {
        new_level = if old_level < 5 { old_level + 1 } else { old_level };

        if similarity >= 0.99 {
            feedback = format!("Mükemmel! Seviye: {} → {}", old_level, new_level);
        } else if close_match {
            feedback = format!(
                "Doğru! (Tam yazılışı: \"{}\") Seviye: {} → {}",
                correct_answer, old_level, new_level
            );
        } else {
            feedback = format!("Doğru! Seviye: {} → {}", old_level, new_level);
        }
    } else {
        new_level = if old_level > 1 { old_level - 1 } else { old_level };

        if close_match {
            feedback = format!(
                "Neredeyse! Doğru cevap: \"{}\" ({}). Seviye: {} → {}",
                correct_answer, foreign, old_level, new_level
            );
        } else {
            feedback = format!(
                "Yanlış! Doğru cevap: \"{}\". Seviye: {} → {}",
                correct_answer, old_level, new_level
            );
        }
    }

    let result = CheckResult {
        correct: is_correct,
        similarity,
        close_match,
        correct_answer: correct_answer.to_string(),
        old_level,
        new_level,
        feedback,
    };

    serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
}
