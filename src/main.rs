use eframe::egui;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;

// ==================== EMBEDDED DATA ====================
mod embedded {
    use std::collections::BTreeMap;

    // LOGO - compile-time'da gömülür
    pub const LOGO_BYTES: &[u8] = include_bytes!("../assets/logo.png");

    // Hazırlık (1..12)
    pub const HAZIRLIK_1: &str = include_str!("../words/hazırlık/unit1.json");
    pub const HAZIRLIK_2: &str = include_str!("../words/hazırlık/unit2.json");
    pub const HAZIRLIK_3: &str = include_str!("../words/hazırlık/unit3.json");
    pub const HAZIRLIK_4: &str = include_str!("../words/hazırlık/unit4.json");
    pub const HAZIRLIK_5: &str = include_str!("../words/hazırlık/unit5.json");
    pub const HAZIRLIK_6: &str = include_str!("../words/hazırlık/unit6.json");
    pub const HAZIRLIK_7: &str = include_str!("../words/hazırlık/unit7.json");
    pub const HAZIRLIK_8: &str = include_str!("../words/hazırlık/unit8.json");
    pub const HAZIRLIK_9: &str = include_str!("../words/hazırlık/unit9.json");
    pub const HAZIRLIK_10: &str = include_str!("../words/hazırlık/unit10.json");
    pub const HAZIRLIK_11: &str = include_str!("../words/hazırlık/unit11.json");
    pub const HAZIRLIK_12: &str = include_str!("../words/hazırlık/unit12.json");

    // 9. 10. sınıf (1..12)
    pub const DOKUZ_ON_1: &str = include_str!("../words/9-10. sınıf/1. ünite (1).json");
    pub const DOKUZ_ON_2: &str = include_str!("../words/9-10. sınıf/2. ünite (2).json");
    pub const DOKUZ_ON_3: &str = include_str!("../words/9-10. sınıf/3. ünite (3).json");
    pub const DOKUZ_ON_4: &str = include_str!("../words/9-10. sınıf/4. ünite (4).json");
    pub const DOKUZ_ON_5: &str = include_str!("../words/9-10. sınıf/5. ünite (5).json");
    pub const DOKUZ_ON_6: &str = include_str!("../words/9-10. sınıf/6. ünite (6).json");
    pub const DOKUZ_ON_7: &str = include_str!("../words/9-10. sınıf/7. ünite (7).json");
    pub const DOKUZ_ON_8: &str = include_str!("../words/9-10. sınıf/8. ünite (8).json");
    pub const DOKUZ_ON_9: &str = include_str!("../words/9-10. sınıf/9. ünite (9).json");
    pub const DOKUZ_ON_10: &str = include_str!("../words/9-10. sınıf/10. ünite (10).json");
    pub const DOKUZ_ON_11: &str = include_str!("../words/9-10. sınıf/11. ünite (11).json");
    pub const DOKUZ_ON_12: &str = include_str!("../words/9-10. sınıf/12. ünite (12).json");

    pub fn embedded_catalog() -> BTreeMap<&'static str, BTreeMap<&'static str, &'static str>> {
        let mut map: BTreeMap<&'static str, BTreeMap<&'static str, &'static str>> = BTreeMap::new();

        // Kategori: "hazırlık"
        let k = "hazırlık";
        map.entry(k).or_default().insert("1", HAZIRLIK_1);
        map.entry(k).or_default().insert("2", HAZIRLIK_2);
        map.entry(k).or_default().insert("3", HAZIRLIK_3);
        map.entry(k).or_default().insert("4", HAZIRLIK_4);
        map.entry(k).or_default().insert("5", HAZIRLIK_5);
        map.entry(k).or_default().insert("6", HAZIRLIK_6);
        map.entry(k).or_default().insert("7", HAZIRLIK_7);
        map.entry(k).or_default().insert("8", HAZIRLIK_8);
        map.entry(k).or_default().insert("9", HAZIRLIK_9);
        map.entry(k).or_default().insert("10", HAZIRLIK_10);
        map.entry(k).or_default().insert("11", HAZIRLIK_11);
        map.entry(k).or_default().insert("12", HAZIRLIK_12);

        // Kategori: "9-10. sınıf"
        let k = "9-10. sınıf";
        map.entry(k).or_default().insert("1", DOKUZ_ON_1);
        map.entry(k).or_default().insert("2", DOKUZ_ON_2);
        map.entry(k).or_default().insert("3", DOKUZ_ON_3);
        map.entry(k).or_default().insert("4", DOKUZ_ON_4);
        map.entry(k).or_default().insert("5", DOKUZ_ON_5);
        map.entry(k).or_default().insert("6", DOKUZ_ON_6);
        map.entry(k).or_default().insert("7", DOKUZ_ON_7);
        map.entry(k).or_default().insert("8", DOKUZ_ON_8);
        map.entry(k).or_default().insert("9", DOKUZ_ON_9);
        map.entry(k).or_default().insert("10", DOKUZ_ON_10);
        map.entry(k).or_default().insert("11", DOKUZ_ON_11);
        map.entry(k).or_default().insert("12", DOKUZ_ON_12);

        map
    }
}

// ==================== APP TYPES ====================
#[derive(PartialEq)]
enum Screen {
    Menu,
    SelectCategory,
    SelectSet,
    AddWords,
    Game,
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Word {
    foreign: String,
    translation: String,
    level: u8,
}

impl Word {
    fn new(foreign: String, translation: String) -> Self {
        Self { foreign, translation, level: 1 }
    }
}

struct App {
    screen: Screen,
    words: Vec<Word>,

    // Add words
    new_foreign: String,
    new_translation: String,

    // Game
    current_word_index: usize,
    user_answer: String,
    feedback_message: String,
    feedback_color: egui::Color32,

    // Multi-set
    available_categories: Vec<String>,
    selected_category: Option<String>,
    available_sets: Vec<String>,
    current_set_name: Option<String>,

    // Logo texture
    logo_texture: Option<egui::TextureHandle>,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            screen: Screen::Menu,
            words: Vec::new(),
            new_foreign: String::new(),
            new_translation: String::new(),
            current_word_index: 0,
            user_answer: String::new(),
            feedback_message: String::new(),
            feedback_color: egui::Color32::WHITE,
            available_categories: Vec::new(),
            selected_category: None,
            available_sets: Vec::new(),
            current_set_name: None,
            logo_texture: None,
        };
        app.load_user_progress();
        app
    }
}

// ==================== MAIN ====================
fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native("GİKAL Wortmeister", options, Box::new(|_cc| Ok(Box::new(App::default()))))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| match self.screen {
            Screen::Menu => self.menu_screen(ui, ctx),
            Screen::SelectCategory => self.category_screen(ui),
            Screen::SelectSet => self.set_screen(ui),
            Screen::AddWords => self.add_words_screen(ui),
            Screen::Game => self.game_screen(ui),
            Screen::End => self.end_screen(ui),
        });
    }
}

// ==================== PROGRESS FILE ====================
impl App {
    fn get_exe_dir() -> std::path::PathBuf {
        env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| ".".into())
    }
    fn get_progress_file_path() -> std::path::PathBuf {
        Self::get_exe_dir().join("user_progress.json")
    }

    fn save_user_progress(&self) {
        let progress_path = Self::get_progress_file_path();
        if let Ok(json) = serde_json::to_string_pretty(&self.words) {
            if let Err(e) = fs::write(&progress_path, json) {
                eprintln!("İlerleme kaydedilemedi {:?}: {}", progress_path, e);
            }
        }
    }

    fn load_user_progress(&mut self) {
        let progress_path = Self::get_progress_file_path();
        if let Ok(data) = fs::read_to_string(&progress_path) {
            if let Ok(vec) = serde_json::from_str::<Vec<Word>>(&data) {
                self.words = vec;
                if !self.words.is_empty() {
                    self.current_word_index = 0;
                }
            }
        }
    }

    // Logo yükleme fonksiyonu
    fn load_logo(&mut self, ctx: &egui::Context) {
        if self.logo_texture.is_none() {
            if let Ok(img) = image::load_from_memory(embedded::LOGO_BYTES) {
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    size,
                    pixels.as_slice(),
                );
                self.logo_texture = Some(ctx.load_texture(
                    "logo",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }
}

// ==================== EMBEDDED SET LOGIC ====================
impl App {
    fn scan_categories(&mut self) {
        self.available_categories.clear();
        for cat in embedded::embedded_catalog().keys() {
            self.available_categories.push((*cat).to_string());
        }
        self.available_categories.sort();
    }

    fn scan_sets_in_category(&mut self, category: &str) {
        self.available_sets.clear();
        if let Some(sets) = embedded::embedded_catalog().get(category) {
            for set_name in sets.keys() {
                self.available_sets.push((*set_name).to_string());
            }
            self.available_sets.sort_by_key(|s| s.parse::<u32>().ok());
        }
    }

    fn load_set(&mut self, category: &str, set_name: &str) {
        if let Some(sets) = embedded::embedded_catalog().get(category) {
            if let Some(json_str) = sets.get(set_name) {
                match serde_json::from_str::<Vec<Word>>(json_str) {
                    Ok(vec) => {
                        self.words = vec;
                        self.current_set_name = Some(format!("{}/{}", category, set_name));
                        if !self.words.is_empty() {
                            self.current_word_index = 0;
                            self.screen = Screen::Game;
                            self.feedback_message.clear();
                            self.user_answer.clear();
                            self.pick_random_word();
                        }
                    }
                    Err(e) => self.feedback_message = format!("JSON hatası: {}", e),
                }
                return;
            }
        }
        self.feedback_message = "Set bulunamadı.".to_string();
    }
}

// ==================== UI SCREENS ====================
impl App {
    fn menu_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Logo yükle
        self.load_logo(ctx);

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);

            // Logo göster
            if let Some(texture) = &self.logo_texture {
                let size = egui::vec2(200.0, 200.0);
                ui.add(egui::Image::new(texture).max_size(size));
                ui.add_space(10.0);
            }

            ui.heading(egui::RichText::new("📚 GİKAL Wortmeister 📚").size(48.0));
            ui.add_space(30.0);

            if ui.button(egui::RichText::new("📂 Kelime Setlerini Seç").size(20.0)).clicked() {
                self.scan_categories();
                self.screen = Screen::SelectCategory;
            }

            ui.add_space(10.0);
            if ui.button(egui::RichText::new("➕ Manuel olarak kelime ekle").size(20.0)).clicked() {
                self.screen = Screen::AddWords;
            }

            ui.add_space(10.0);
            if !self.words.is_empty() {
                let label = if let Some(set) = &self.current_set_name {
                    format!("▶ Devam Et ({})", set)
                } else {
                    "▶ Devam Et".to_string()
                };
                if ui.button(egui::RichText::new(label).size(20.0)).clicked() {
                    self.screen = Screen::Game;
                    self.pick_random_word();
                }
            }

            ui.add_space(20.0);
            ui.label(egui::RichText::new("Mete PARLAK tarafından yazıldı."));
            ui.label(egui::RichText::new("Bu proje woro projesinin GİKAL için yapılmış forkudur.").size(12.0).weak());
        });
    }

    fn category_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Sınıfını Seç");
            ui.add_space(10.0);

            if self.available_categories.is_empty() {
                ui.label("Gömülü kategori bulunamadı.");
            } else {
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for cat in self.available_categories.clone() {
                        if ui.button(egui::RichText::new(&cat).size(18.0)).clicked() {
                            self.selected_category = Some(cat.clone());
                            self.scan_sets_in_category(&cat);
                            self.screen = Screen::SelectSet;
                        }
                    }
                });
            }

            ui.add_space(20.0);
            if ui.button("⬅ Ana Menü").clicked() {
                self.screen = Screen::Menu;
            }
        });
    }

    fn set_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            if let Some(cat) = &self.selected_category.clone() {
                ui.heading(format!("Ünite Seç: {}", cat));
                ui.add_space(10.0);

                if self.available_sets.is_empty() {
                    ui.label("Bu kategoride set bulunamadı.");
                } else {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for set in self.available_sets.clone() {
                            let button_text = format!("{}. Ünite", set);
                            if ui.button(egui::RichText::new(&button_text).size(18.0)).clicked() {
                                self.load_set(cat, &set);
                            }
                        }
                    });
                }

                ui.add_space(20.0);
                if ui.button("⬅ Sınıf Seç").clicked() {
                    self.screen = Screen::SelectCategory;
                }

                if !self.feedback_message.is_empty() {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(&self.feedback_message).color(egui::Color32::RED));
                }
            }
        });
    }

    fn add_words_screen(&mut self, ui: &mut egui::Ui) {
        ui.heading("Kelime Ekle");
        ui.add_space(10.0);

        if ui.button("📁 TXT'den yükle").clicked() {
            self.import_from_txt();
        }

        ui.add_space(10.0);
        egui::Grid::new("add_word_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Yabancı kelime:");
            ui.text_edit_singleline(&mut self.new_foreign);
            ui.end_row();

            ui.label("Çeviri:");
            let response = ui.text_edit_singleline(&mut self.new_translation);
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.add_word();
            }
            ui.end_row();
        });

        ui.add_space(8.0);
        if ui.button("➕ Kelime ekle").clicked() {
            self.add_word();
        }

        if !self.words.is_empty() {
            if ui.button("🎮 Oyuna git").clicked() {
                self.feedback_message.clear();
                self.screen = Screen::Game;
                self.pick_random_word();
            }
        }

        ui.add_space(10.0);
        if ui.button("⬅ Ana Menü").clicked() {
            self.screen = Screen::Menu;
        }

        ui.separator();
        ui.heading("Kelimeleriniz:");

        if self.words.is_empty() {
            ui.label("Kelime yok. Ezberlemeye başlamak için birkaç kelime ekleyin!");
        } else {
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                let mut to_delete: Option<usize> = None;

                for (i, word) in self.words.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("🔹 {} = {}", word.foreign, word.translation));
                        ui.label(format!("(Seviye {})", word.level));
                        if ui.button("🗑 Sil").clicked() {
                            to_delete = Some(i);
                        }
                    });
                }

                if let Some(index) = to_delete {
                    self.words.remove(index);
                    self.save_user_progress();
                }
            });
        }
    }

    fn game_screen(&mut self, ui: &mut egui::Ui) {
        if self.words.is_empty() {
            ui.heading("Kelime yok!");
            ui.label("İlk önce kelime ekleyin.");
            return;
        }

        ui.heading("🎮 Oyun");
        if let Some(set) = &self.current_set_name {
            ui.label(egui::RichText::new(format!("Set: {}", set)).weak());
        }
        ui.add_space(6.0);

        let mastered = self.words.iter().filter(|w| w.level >= 5).count();
        let total = self.words.len();
        let progress = mastered as f32 / (total as f32).max(1.0);
        ui.horizontal(|ui| {
            ui.label("Ezberlenenler:");
            ui.add(egui::ProgressBar::new(progress).text(format!("{}/{} ezberlendi", mastered, total)).desired_width(220.0));
        });

        ui.separator();
        ui.add_space(10.0);

        let word = &self.words[self.current_word_index];
        ui.label("Bu kelimenin çevirisi nedir?");
        ui.label(egui::RichText::new(&word.foreign).size(48.0).strong());
        ui.label(format!("Seviye: {}", word.level));

        ui.add_space(12.0);
        ui.label("Cevabınız:");
        let response = ui.text_edit_singleline(&mut self.user_answer);

        if response.changed() {
            self.feedback_message.clear();
        }

        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.check_answer();
        }

        if ui.button("✓ Kontrol et").clicked() {
            self.check_answer();
        }

        ui.add_space(10.0);
        if !self.feedback_message.is_empty() {
            ui.label(
                egui::RichText::new(&self.feedback_message)
                    .size(18.0)
                    .color(self.feedback_color)
            );
        }

        ui.add_space(20.0);
        if ui.button("⬅ Ana Menü").clicked() {
            self.screen = Screen::Menu;
        }
    }

    fn end_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("🎉🎉🎉").size(72.0));
            ui.add_space(12.0);
            ui.label(egui::RichText::new("TEBRİKLER!").size(40.0).strong().color(egui::Color32::GOLD));
            ui.add_space(8.0);
            ui.label(egui::RichText::new("BÜTÜN KELİMELER EZBERLENDİ!").size(26.0).color(egui::Color32::GREEN));

            ui.add_space(20.0);
            ui.label(egui::RichText::new(format!("{} kelime ezberlediniz!", self.words.len())).size(18.0));

            ui.add_space(30.0);
            if ui.button(egui::RichText::new("🔄 Tekrar oyna").size(18.0)).clicked() {
                for w in &mut self.words {
                    w.level = 1;
                }
                self.save_user_progress();
                self.screen = Screen::Game;
                self.pick_random_word();
            }

            if ui.button(egui::RichText::new("➕ Daha fazla kelime ekle").size(18.0)).clicked() {
                self.screen = Screen::AddWords;
            }

            ui.add_space(10.0);
            if ui.button("⬅ Ana Menü").clicked() {
                self.screen = Screen::Menu;
            }

            ui.add_space(10.0);
            ui.label("Wortmeister'i kullandığınız için teşekkürler!");
        });
    }
}

// ==================== CORE LOGIC ====================
impl App {
    fn add_word(&mut self) {
        if !self.new_foreign.trim().is_empty() && !self.new_translation.trim().is_empty() {
            self.words.push(Word::new(self.new_foreign.trim().to_string(), self.new_translation.trim().to_string()));
            self.new_foreign.clear();
            self.new_translation.clear();
            self.save_user_progress();
        }
    }

    fn import_from_txt(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Text Files", &["txt"])
            .set_title("Kelime listesi seç")
            .pick_file() 
        {
            match fs::read_to_string(path) {
                Ok(content) => self.parse_txt_content(&content),
                Err(e) => eprintln!("Dosya okunamadı: {}", e),
            }
        }
    }

    fn parse_txt_content(&mut self, content: &str) {
        let mut added = 0usize;
        let mut skipped = 0usize;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let foreign = parts[0].to_string();
                let translation = parts[1..].join(" ");
                self.words.push(Word::new(foreign, translation));
                added += 1;
            } else {
                skipped += 1;
            }
        }
        if added > 0 {
            self.save_user_progress();
        }
        println!("✅ {} kelime eklendi, {} satır atlandı", added, skipped);
    }

    fn pick_random_word(&mut self) {
        if self.words.is_empty() {
            return;
        }
        let mut rng = rand::thread_rng();
        self.current_word_index = rng.gen_range(0..self.words.len());
    }

    fn check_answer(&mut self) {
        let idx = self.current_word_index;
        let correct_translation = self.words[idx].translation.clone();
        let old_level = self.words[idx].level;
        let user = self.user_answer.trim().to_lowercase();
        let low = correct_translation.to_lowercase();
        let right = low.replace(" ", "");

        if user == right {
            let w = &mut self.words[idx];
            if w.level < 5 {
                w.level += 1;
                self.feedback_message = format!("✅ DOĞRU! Seviye: {} → {}", old_level, w.level);
            } else {
                self.feedback_message = "✅ DOĞRU! Zaten ezberlendi!".to_string();
            }
            self.feedback_color = egui::Color32::GREEN;
        } else {
            let w = &mut self.words[idx];
            if w.level > 1 {
                w.level -= 1;
            }
            self.feedback_message =
                format!("❌ YANLIŞ! Doğru cevap: {} (Seviye: {} → {})", correct_translation, old_level, w.level);
            self.feedback_color = egui::Color32::RED;
        }

        self.save_user_progress();
        self.pick_random_word();

        if self.all_words_mastered() {
            self.screen = Screen::End;
            self.feedback_message.clear();
        }

        self.user_answer.clear();
    }

    fn all_words_mastered(&self) -> bool {
        !self.words.is_empty() && self.words.iter().all(|w| w.level >= 5)
    }
}

