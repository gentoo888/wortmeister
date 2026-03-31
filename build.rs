use std::path::Path;

fn main() {
    let words_dir = Path::new("words");
    println!("cargo:rerun-if-changed={}", words_dir.display());
    if words_dir.exists() {
        for entry in walkdir(words_dir) {
            println!("cargo:rerun-if-changed={}", entry.display());
        }
    }
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}