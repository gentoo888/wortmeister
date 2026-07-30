<h1>◣◢</h1>

For the english README:
https://gentoo888.github.io/mevicta/wortmeister.html

Learning Journal: Still on it

# GIKAL Wortmeister

## Proje Hakkında

GIKAL Wortmeister, Almanca kelime ezberleme sürecini aralıklı tekrar ve seviye bazlı ilerleme ile destekleyen, web tabanlı bir uygulamadır. Proje, Göztepe İhsan Kurşunoğlu Anadolu Lisesi öğrencileri için hazırlanmıştır.

Üç ana bileşenden oluşur:

1. **Rust tabanlı HTTP sunucu**
2. **Rust WebAssembly çekirdeği**
3. **HTML/CSS/JS frontend**

Henüz bunun için altyapı tam olarak hazırlanmamış olsa da, hazırlandığında kullanıcılar kayıt olup giriş yaparak ilerlemelerini sunucuda saklayabilir, misafir olarak devam edip verilerini tarayıcıda tutabilir veya manuel olarak kendi kelime listelerini oluşturup çalışabilir.

---

## Mimari

Kullanıcı girdileri tarayıcısından gönderir, girdiler Rust Axum sunucusuna taşınır ve users.json'a yazılır. Sonrasında index.html styles.css WASM gibi statik dosyalarla birlikte WebAssembly modülüne aktarılır. include_dir! ile gömülmüş kelime kataloğu ve süreç içerisinde normalize edilmiş kelimelerle kontrol sağlanır.

### Katmanların Sorumlulukları

#### Sunucu (`main.rs`)

- Kullanıcı hesap yönetimi
- UUID token ile oturum doğrulama
- İlerleme ve istatistik verilerinin JSON dosyasında saklanması
- `tower_http::services::ServeDir` ile statik frontend dosyalarının sunulması

#### WASM Çekirdeği (`lib.rs`) (Rust tarafında asıl olay burada oluyor)

- Derleme zamanında `words/` altındaki tüm JSON kelime dosyalarını kataloğa dönüştürür
- JavaScript’ten çağrılacak fonksiyonları sağlar:
  - `get_categories`, `get_sets`, `get_words`, `get_category_name`
  - `check_answer`
- Cevap kontrolü adımları:
  1. Ham ve normalize edilmiş eşleşme kontrolleri
  2. Almanca artikelleri temizlenir (İlk başta gereksiz geldi ama sonra garanti olsun dedim ekledim)
  3. Çoklu alternatif desteği
  4. Parantez içi açıklamaların yok sayılması
  5. `normalized_levenshtein` ile benzerlik puanlaması (%85 doğru, %65 yakın sınır şeklinde).
- Her cevap sonucunda kelime seviyesi 1 ila 5 arasında güncellenir

#### Frontend (klasik CSS, HTML, JS üçlüsü)

- Ekran geçişleri (auth, menu, category, set, game, end, addWords...)
- `sessionStorage` / `localStorage` ile oturum ve misafir ilerlemesi
- Kimlik doğrulama sonrası ilerlemeyi sunucuya kaydeder (`/api/auth/save`)
- WASM modülü ile senkronize iletişim
- Responsive tasarım, özel CSS animasyonları (bitiş ekranında konfeti, seri patlaması, (yanlış cevaplandığında)kutu sallama).

## Sunucu API Referansı

Tüm istekler `POST` metodunu kullanır ve `Content-Type: application/json` gerektirir

## WASM Fonksiyon Referansı

`wasm-bindgen` aracılığıyla dışa aktarılan tüm fonksiyonlar JSON string döndürür

## Cevap Kontrol Algoritması

Bu işle görevli olan `check_answer` fonksiyonu sırasıyla şu kontrolleri yapar:

1. **Doğrudan eşleşme:** Kullanıcı cevabı `trim().to_lowercase()` ile doğru cevapla karşılaştırılır
2. **Boşluksuz eşleşme:** Tüm boşluklar çıkarılarak karşılaştırma tekrarlanır
3. **NFKD normalizasyonu:** Her iki metin Unicode NFKD ile ayrıştırılır, ardından Almanca/Türkçe özel karakterler ASCII eşdeğerlerine dönüştürülür (ä→ae, ß→ss, ş→s vb.) Normalize edilmiş metinler karşılaştırılır
4. **Artikel temizleme:** `strip_article()` ile Almanca belirli belirsiz artikeller kaldırılır kalan kısım hem ham hem de normalize edilerek karşılaştırılır
5. **Alternatif çeviriler:** Doğru cevap `/` karakteri içeriyorsa her bir alternatif için normalizasyonlu ve benzerlik kontrolleri yapılır Benzerlik ≥ 0.85 ise doğru kabul edilir (Klasik MEB yöntemi seçtim XD)
6. **Parantez içi açıklamalar:** Doğru cevapta `(` varsa, parantezden önceki kısım aynı yöntemlerle kontrol edilir
7. **Levenshtein benzerliği:** Hiçbiri tutmazsa `normalized_levenshtein` ile benzerlik hesaplanır. ≥ 0.85 doğru, ≥ 0.65 yakın kabul edilir

Seviye güncellemesi:

- Doğru: `old_level` 5’ten küçükse 1 artar.
- Yanlış: `old_level` 1’den büyükse 1 azalır.
- Böylelikle yeni seviye her zaman 1 ile 5 arasında kalır.

---

## İlerleme ve İstatistik Veri Yapısı

### `progress`

Anahtarlar kategori ve set ID’sinin birleşimi şeklindedir (örn. `"hazirlik_1"`). Her giriş:

```json
{
  "category": "Hazırlık",
  "setName": "Hazırlık / 1. Ünite",
  "masteredCount": 42,
  "totalCount": 50,
  "updatedAt": "2025-01-01T12:00:00.000Z",
  "words": [
    { "foreign": "der Hund", "translation": "köpek", "level": 5 },
    ...
  ]
}
```

### `stats` Nesnesi

```json
{
  "bestStreak": 15,
  "totalCorrect": 120,
  "totalAnswered": 145
}
```

Giriş yapmış kullanıcılarda bu veriler hem sunucuya (`/api/auth/save`) hem de yedek olarak `localStorage`'a kaydedilir Misafirlerde kaydetecek kullanıcı alanı olmaığı için yalnızca `localStorage` kullanılır

---

## Derleme ve Çalıştırma

Kim alır da derler bunu bilmem ama yazayım dedim

### 1. WASM modülünü derle

```bash
wasm-pack build --target web --out-dir static/pkg
```

Bu komut `static/pkg/` altına gerekli `.wasm` ve `.js` bağlayıcısını oluşturur

### 2. Sunucuyu derle

```bash
cargo build --release --bin wortmeister-server
```

### 3. Sunucuyu başlat

```bash
USERS_FILE=users.json STATIC_DIR=../static PORT=9090 ./target/release/wortmeister-server
```

Ortam değişkenleri:

- `USERS_FILE`: Kullanıcı veritabanı dosyası (`users.json`)
- `STATIC_DIR`: Statik dosyaların (frontend falan) bulunduğu dizin (`../static`)
- `PORT`: Sunucu portu (varsayılan: `9090`)

### 4. Erişim

## Tarayıcıya port girilerek erişim sağlanır

## Ön Yüz Ekranları

- **authScreen**: Kayıt giriş ve misafir devam seçenekleri
- **menuScreen**: Ana menü, set seçimi, manuel kelime ekleme, devam et, çıkış...
- **categoryScreen**: Sınıf, kategori seçimi
- **setScreen**: Ünite seti seçimi
- **gameScreen**: Kelime kartı, seviye göstergesi, cevap girişi, geri bildirim, ilerleme çubuğu...
- **endScreen**: Tüm kelimeler halledildiğinde tebrik ekranı ve konfeti
- **addWordsScreen**: Kullanıcının kendi kelime listesini oluşturması

---

## Güvenlik Notları (Çünkü gelecekte Donanım Güvenlikçisi olmak istiyorum)

- Şifreler `SHA256(username + ":" + password)` ile hashlenir Salt kullanılmadığı için üretimde bcrypt ya da argon2 öneririm
- Oturum tokenları UUID’dir ve düz metin saklanır Hassas veri içermeyen bir eğitim uygulaması için bence yeterli
- CORSu tüm kaynaklara izin verecek şekilde ayarlamaya gayret gösterdim (`CorsLayer::permissive()`) Yalnızca iç ağda kullanıma uygundur

---

## Lisans ve Katkı
MIT License

Bu proje Göztepe İhsan Kurşunoğlu Anadolu Lisesi öğrencilerinin özellikle hazırlık sınıfındakilere her hafta olan kelime sınavlarına hazırlanmalarını kolaylaştırmak ve bir nebze de olsa eğlenceli hale getirmek için için **Mete PARLAK** tarafından oluşturulmuştur. PR'lara her zaman açığım ve memnuniyet duyarım. Woro projemin bir forkudur. Eksik veya hatalı kelime bildirimleri için lütfen `metep788@gmail.com`'a bildirin.
