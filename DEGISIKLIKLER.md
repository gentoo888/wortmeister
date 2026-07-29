# Wortmeister - WASM Dönüşümü ve Yenilikler

Bu belge, Wortmeister uygulamasının klasik API mimarisinden WebAssembly (WASM) mimarisine geçişi sırasında yapılan tüm değişiklikleri ve eklenen yenilikleri detaylı olarak açıklar.

## 1. Mimari Değişiklik: Klasik API'den WASM'a Geçiş

### Eski Mimari
Uygulamanın önceki sürümünde tüm oyun mantığı sunucu tarafında çalışıyordu. Frontend, her işlem için `https://wortmeister.onrender.com` adresindeki Rust (axum) sunucusuna HTTP istekleri gönderiyordu:

- Kategori listesi: `GET /api/categories`
- Ünite listesi: `GET /api/categories/:id/sets`
- Oyun başlatma: `POST /api/game/start`
- Cevap kontrolü: `POST /api/game/check`
- Oturum yönetimi: `GET /api/game/session/:id`, `POST /api/game/reset`, `POST /api/game/add-word`, `POST /api/game/custom-session`

Bu yaklaşımın sorunları:
- Her cevap kontrolü için ağ gecikmesi yaşanıyordu
- Render free tier nedeniyle sunucu uykuya geçtiğinde kategoriler geç yükleniyordu
- İnternet bağlantısı olmadan oyun oynanamıyordu
- Sunucu tarafında oturum durumu bellekte tutulduğu için sunucu yeniden başladığında oturumlar kayboluyordu

### Yeni Mimari
Tüm oyun mantığı Rust ile yazılmış bir WASM modülüne taşındı (`wasm/src/lib.rs`). Bu modül `wasm-bindgen` ile derlenip tarayıcıda çalışır:

- `get_categories()`: Kategori listesini döndürür
- `get_sets(category_id)`: Bir kategorinin ünitelerini döndürür
- `get_words(category_id, set_id)`: Bir ünitenin kelimelerini döndürür
- `get_category_name(category_id)`: Kategori adını döndürür
- `check_answer(user_answer, correct_answer, foreign, old_level)`: Cevap kontrolü yapar, benzerlik puanını hesaplar, yeni seviyeyi ve geri bildirim metnini üretir

Tüm kelime verileri (36 ünite, 3 kategori, toplam 3700'den fazla kelime) `include_dir` makrosu ile derleme zamanında WASM ikili dosyasının içine gömüldü. Böylece:

- Cevap kontrolü artık anında, ağ gecikmesi olmadan gerçekleşir
- Kategoriler ve üniteler beklemeden yüklenir
- Sayfa bir kez yüklendikten sonra oyun tamamen çevrimdışı oynanabilir
- Levenshtein benzerlik hesaplama (`strsim`), Unicode normalizasyonu (`unicode-normalization`), artikel ayıklama ve alternatif cevap eşleştirme gibi tüm akıllı cevap kontrol mantığı Rust'ta kaldı ve birebir korundu

Katalog verisi `std::sync::OnceLock` ile önbelleğe alınır; böylece her fonksiyon çağrısında JSON dosyaları yeniden ayrıştırılmaz.

## 2. Kullanıcı Hesabı Sistemi ve Kalıcı İlerleme

Kullanıcı ilerlemesinin uygulama kapatılıp açıldığında kaybolmaması için kullanıcı adı ve şifre tabanlı bir hesap sistemi eklendi.

### Sunucu Tarafı (server/src/main.rs)
Eski oyun API'si tamamen kaldırıldı; yerine sadece kimlik doğrulama ve ilerleme kaydı yapan hafif bir axum sunucusu yazıldı:

- `POST /api/auth/register`: Yeni kullanıcı kaydı. Kullanıcı adı en az 3, şifre en az 4 karakter olmalıdır. Aynı kullanıcı adı ikinci kez alınamaz.
- `POST /api/auth/login`: Giriş. Başarılı girişte oturum token'ı, kayıtlı ilerleme ve istatistikler döndürülür.
- `POST /api/auth/save`: İlerleme ve istatistik kaydı. Geçerli token gerektirir.

Tüm kullanıcı verileri `users.json` dosyasına kaydedilir. Dosya yapısı şu şekildedir:

```json
{
  "kullanici_adi": {
    "username": "kullanici_adi",
    "password_hash": "sha256 karması",
    "progress": {
      "hazirlik_1": [
        { "foreign": "der Hund", "translation": "köpek", "level": 3 }
      ]
    },
    "stats": { "bestStreak": 5, "totalAnswered": 120 }
  }
}
```

Güvenlik önlemleri:
- Şifreler asla düz metin olarak saklanmaz; kullanıcı adı ile tuzlanmış SHA-256 karması olarak kaydedilir
- İlerleme kaydı için her girişte üretilen UUID tabanlı oturum token'ı doğrulanır
- Dosya yolu `USERS_FILE` ortam değişkeni ile özelleştirilebilir

### Frontend Tarafı
- Uygulama açılışında giriş/kayıt ekranı gösterilir
- "Giriş Yap", "Kayıt Ol" ve "Misafir Olarak Devam Et" seçenekleri sunulur
- Giriş yapan kullanıcının ilerlemesi her cevaptan sonra otomatik olarak sunucuya senkronize edilir
- Misafir kullanıcıların ilerlemesi eskisi gibi localStorage'da saklanır
- Ana menüde "Hoş geldin, kullanıcı!" mesajı ve "Çıkış Yap" düğmesi gösterilir
- Şifre alanında Enter tuşu ile giriş yapılabilir
- Bir ünite tekrar açıldığında kayıtlı seviyeler otomatik olarak geri yüklenir

## 3. Logo Hizalama Sorununun Düzeltilmesi

Ana ekrandaki okul logosu (`gikal.png`) HTML'de herhangi bir CSS sınıfı olmadan `<img>` etiketi olarak duruyordu. Satır içi (inline) bir öğe olduğu için blok kapsayıcının sol kenarına yaslanıyor ve sola kaymış görünüyordu.

Çözüm olarak `styles.css` dosyasına `.menu-logo-img` sınıfı eklendi:

```css
.menu-logo-img {
  display: block;
  width: 120px;
  height: 120px;
  margin: 40px auto 20px;
  object-fit: contain;
}
```

`display: block` ve `margin: auto` sayesinde logo artık her ekran boyutunda tam ortalanır. Mevcut tasarımın geri kalanına dokunulmadı; sadece yeni eklenen giriş ekranı ve kullanıcı bilgisi için gerekli minimal stiller dosyanın sonuna eklendi.

## 4. Türkçe Karakter Düzeltmeleri

Frontend'deki tüm metinler doğru Türkçe karakterlerle yeniden yazıldı. Düzeltme yapılan yerlerden bazıları:

| Eski | Yeni |
|------|------|
| Kelime Setlerini Sec | Kelime Setlerini Seç |
| Sinifini Sec | Sınıfını Seç |
| Calismak istedigin sinifi sec | Çalışmak istediğin sınıfı seç |
| Unite Sec | Ünite Seç |
| Calismak istedigin uniteyi sec | Çalışmak istediğin üniteyi seç |
| Bu kelimenin Turkce cevirisi nedir? | Bu kelimenin Türkçe çevirisi nedir? |
| Cevabinizi yazin... | Cevabınızı yazın... |
| Gec / Ipucu / Ana Menu | Geç / İpucu / Ana Menü |
| TEBRIKLER! / Butun kelimeler ezberlendi! | TEBRİKLER! / Bütün kelimeler ezberlendi! |
| Tekrar Oyna / Baska Set Sec | Tekrar Oyna / Başka Set Seç |
| Kendi kelime listeni olustur | Kendi kelime listeni oluştur |
| Turkce Ceviri / kopek | Türkçe Çeviri / köpek |
| Oyuna Basla | Oyuna Başla |
| Henuz kelime eklenmedi... | Henüz kelime eklenmedi... |
| Mete PARLAK tarafindan olusturuldu. | Mete PARLAK tarafından oluşturuldu. |
| GIKAL icin yapilmis forkudur | GİKAL için yapılmış forkudur |
| unite / kelime | ünite / kelime |
| Kategoriler yuklenemedi | (WASM sayesinde bu hata durumu kalktı) |
| Oyun baslatilamadi! | Oyun başlatılamadı! |
| Baglanti hatasi! | (Cevap kontrolü yerel olduğu için kalktı) |
| Gecildi. Cevap: | Geçildi. Cevap: |
| Ipucu: | İpucu: |
| Her iki alani da doldurun! | Her iki alanı da doldurun! |
| Ozel Kelime Listesi | Özel Kelime Listesi |
| Dogruluk | Doğruluk |

Ayrıca WASM modülündeki kategori adları ve geri bildirim metinleri de Türkçe karakterlerle güncellendi: "Hazirlik" → "Hazırlık", "Hazirlik 2. Donem" → "Hazırlık 2. Dönem", "9-10. Sinif" → "9-10. Sınıf", "Mukemmel" → "Mükemmel", "Dogru" → "Doğru", "Yanlis" → "Yanlış", "Neredeyse" gibi.

Sayfa başlığı da "GIKAL Wortmeister" yerine "GİKAL Wortmeister" yapıldı.

## 5. Emojilerin Kaldırılması

Eski sunucu kodundaki geri bildirim metinlerinde bulunan tüm emojiler kaldırıldı:

- "✅ Mükemmel!" → "Mükemmel!"
- "✅ Doğru!" → "Doğru!"
- "❌ Yanlış!" → "Yanlış!"
- "❌ Neredeyse!" → "Neredeyse!"
- Konsol çıktılarındaki "📚" ve "🚀" emojileri kaldırıldı

Doğru/yanlış durumu emoji yerine mevcut renk kodlaması ile iletilir (yeşil, kırmızı, sarı geri bildirim kutuları).

## 6. Kaldırılan Gereksiz İçerikler

- "Bu website render free tier ile deploy edildigi icin kategorilerin yuklenmesi biraz zaman alabilir" uyarısı kaldırıldı; çünkü WASM mimarisinde kategoriler anında yüklenir
- Harici `ninja-daytona-script.js` betiği kaldırıldı
- Sunucuya bağımlı olan `custom-session`, `add-word` ve `reset` API çağrıları kaldırıldı; özel kelime listesi oyunu artık tamamen tarayıcıda çalışır

## 7. Proje Yapısı

```
wortmeister-wasm/
├── wasm/                    Rust WASM modülü (oyun mantığı)
│   ├── Cargo.toml
│   └── src/lib.rs
├── server/                  Rust axum sunucusu (kimlik doğrulama + statik dosyalar)
│   ├── Cargo.toml
│   └── src/main.rs
├── words/                   Kelime verileri (WASM içine gömülür)
│   ├── hazirlik/            unite1.json ... unite12.json
│   ├── hazirlik2_donem/     unite1.json ... unite12.json
│   └── 9_10_sinif/          unite1.json ... unite12.json
├── static/                  Frontend
│   ├── index.html
│   ├── app.js               ES module, WASM'ı yükler ve kullanır
│   ├── styles.css
│   ├── gikal.png
│   └── pkg/                 wasm-bindgen çıktısı
│       ├── wortmeister_wasm.js
│       └── wortmeister_wasm_bg.wasm
├── users.json               Kullanıcı hesapları ve ilerleme (çalışma anında oluşur)
└── DEGISIKLIKLER.md         Bu belge
```

## 8. Derleme ve Çalıştırma

WASM modülünü derlemek için:

```bash
cd wasm
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/wortmeister_wasm.wasm \
  --out-dir ../static/pkg --target web
```

Sunucuyu derleyip çalıştırmak için:

```bash
cd server
cargo build --release
./target/release/wortmeister-server
```

Sunucu varsayılan olarak 9090 portunda çalışır (`PORT` ortam değişkeni ile değiştirilebilir) ve `static/` klasörünü sunar. Kullanıcı verileri `users.json` dosyasına yazılır (`USERS_FILE` ortam değişkeni ile değiştirilebilir).

## 9. Korunan Özellikler

Aşağıdaki özellikler ve davranışlar birebir korundu:

- Mevcut tasarım, renk paleti ve tüm CSS bileşenleri
- Akıllı cevap kontrolü: Unicode normalizasyonu, Türkçe/Almanca karakter eşleştirme (ä→ae, ö→oe, ü→ue, ş→s, ç→c, ğ→g, ı→i), artikel ayıklama (der/die/das vb.), "/" ile ayrılmış alternatif cevaplar, parantezli açıklamaların yok sayılması, Levenshtein benzerlik eşikleri (0.85 kabul, 0.65 yakın)
- 5 seviyeli kelime ustalaşma sistemi ve ağırlıklı rastgele kelime seçimi
- Seri (streak) takibi ve animasyonları
- İpucu sistemi (ilk harf, ardından %60 gösterim)
- Klavye kısayolları (Enter ile kontrol, Tab ile geç)
- Manuel kelime ekleme ve özel liste ile oynama
- Oyun sonu ekranı, konfeti ve doğruluk istatistiği
