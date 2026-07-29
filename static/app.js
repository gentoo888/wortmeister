import init, {
  get_categories,
  get_sets,
  get_words,
  get_category_name,
  check_answer as wasmCheckAnswer,
} from "./pkg/wortmeister_wasm.js";

let wasmReady = false;

let auth = {
  username: null,
  token: null,
  guest: false,
};

let state = {
  words: [],
  currentIndex: 0,
  category: "",
  setName: "",
  streak: 0,
  bestStreak: 0,
  totalCorrect: 0,
  totalAnswered: 0,
  customWords: [],
  hintUsed: false,
  progress: {},
  stats: {},
  _categoryId: null,
  _setId: null,
};

async function initWasm() {
  await init();
  wasmReady = true;
}

function showScreen(id) {
  document
    .querySelectorAll(".screen")
    .forEach((s) => s.classList.remove("active"));
  const screen = document.getElementById(id);
  screen.classList.add("active");
  screen.style.animation = "none";
  screen.offsetHeight;
  screen.style.animation = "";
}

function setAuthMessage(msg, isError) {
  const el = document.getElementById("authMessage");
  el.textContent = msg;
  el.className = isError ? "auth-message-error" : "auth-message-success";
}

async function doRegister() {
  const username = document.getElementById("authUsername").value.trim();
  const password = document.getElementById("authPassword").value;
  if (!username || !password) {
    setAuthMessage("Kullanıcı adı ve şifre girin.", true);
    return;
  }
  try {
    const res = await fetch("/api/auth/register", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });
    const data = await res.json();
    if (!data.success) {
      setAuthMessage(data.message, true);
      return;
    }
    onAuthSuccess(username, data);
  } catch (e) {
    setAuthMessage("Sunucuya bağlanılamadı.", true);
    console.error(e);
  }
}

async function doLogin() {
  const username = document.getElementById("authUsername").value.trim();
  const password = document.getElementById("authPassword").value;
  if (!username || !password) {
    setAuthMessage("Kullanıcı adı ve şifre girin.", true);
    return;
  }
  try {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, password }),
    });
    const data = await res.json();
    if (!data.success) {
      setAuthMessage(data.message, true);
      return;
    }
    onAuthSuccess(username, data);
  } catch (e) {
    setAuthMessage("Sunucuya bağlanılamadı.", true);
    console.error(e);
  }
}

function onAuthSuccess(username, data) {
  auth.username = username;
  auth.token = data.token;
  auth.guest = false;
  state.progress = data.progress || {};
  state.stats = data.stats || {};

  sessionStorage.setItem(
    "wortmeister_auth",
    JSON.stringify({ username, token: data.token }),
  );

  const info = document.getElementById("menuUserInfo");
  info.textContent = `Hoş geldin, ${username}!`;
  info.style.display = "block";
  document.getElementById("logoutBtn").style.display = "flex";

  showMenu();
  showToast("Giriş başarılı!", "success");
}

function skipAuth() {
  auth.guest = true;
  auth.username = null;
  auth.token = null;
  state.progress = loadLocalProgress();
  document.getElementById("menuUserInfo").style.display = "none";
  document.getElementById("logoutBtn").style.display = "none";
  showMenu();
}

function doLogout() {
  auth.username = null;
  auth.token = null;
  auth.guest = false;
  state.progress = {};
  sessionStorage.removeItem("wortmeister_auth");
  document.getElementById("authUsername").value = "";
  document.getElementById("authPassword").value = "";
  setAuthMessage("", false);
  showScreen("authScreen");
}

function loadLocalProgress() {
  try {
    return JSON.parse(localStorage.getItem("wortmeister_progress") || "{}");
  } catch (e) {
    return {};
  }
}

function saveLocalProgress() {
  localStorage.setItem("wortmeister_progress", JSON.stringify(state.progress));
}

async function syncProgressToServer() {
  if (auth.guest || !auth.username || !auth.token) {
    saveLocalProgress();
    return;
  }
  try {
    await fetch("/api/auth/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        username: auth.username,
        token: auth.token,
        progress: state.progress,
        stats: state.stats,
      }),
    });
  } catch (e) {
    console.error("Sync failed", e);
  }
}

function showMenu() {
  showScreen("menuScreen");
  updateContinueButton();
}

function updateContinueButton() {
  const btn = document.getElementById("continueBtn");
  if (state.words.length > 0 && state.setName) {
    btn.style.display = "flex";
    btn.textContent = `Devam Et (${state.setName})`;
  } else {
    btn.style.display = "none";
  }
}

function showCategories() {
  showScreen("categoryScreen");
  const grid = document.getElementById("categoryGrid");

  if (!wasmReady) {
    grid.innerHTML = '<div class="loading-spinner"></div>';
    setTimeout(showCategories, 200);
    return;
  }

  const categories = JSON.parse(get_categories());
  const icons = { hazirlik: "1", hazirlik2_donem: "2", sinif_9_10: "3" };

  grid.innerHTML = "";
  categories.forEach((cat, i) => {
    const card = document.createElement("div");
    card.className = "card";
    card.style.animationDelay = `${i * 0.08}s`;
    card.onclick = () => showSets(cat.id, cat.name);
    card.innerHTML = `
            <div class="card-icon">${icons[cat.id] || "•"}</div>
            <div class="card-title">${cat.name}</div>
            <div class="card-info">${cat.set_count} ünite</div>
        `;
    grid.appendChild(card);
  });
}

function showSets(categoryId, categoryName) {
  showScreen("setScreen");
  document.getElementById("setScreenTitle").textContent = `${categoryName}`;
  document.getElementById("setScreenSub").textContent =
    "Çalışmak istediğin üniteyi seç";

  const grid = document.getElementById("setGrid");
  const sets = JSON.parse(get_sets(categoryId));

  grid.innerHTML = "";
  sets.forEach((set, i) => {
    const card = document.createElement("div");
    card.className = "card set-card";
    card.style.animationDelay = `${i * 0.06}s`;
    card.onclick = () => startGame(categoryId, set.id);
    card.innerHTML = `
            <div class="card-icon">${set.id}</div>
            <div class="card-title">${set.name}</div>
            <div class="card-info">${set.word_count} kelime</div>
        `;
    grid.appendChild(card);
  });
}

function startGame(categoryId, setId) {
  const words = JSON.parse(get_words(categoryId, setId));
  if (!words.length) {
    showToast("Oyun başlatılamadı!", "error");
    return;
  }

  const catName = get_category_name(categoryId);
  const progressKey = `${categoryId}_${setId}`;
  const saved = state.progress[progressKey];
  if (saved && Array.isArray(saved)) {
    words.forEach((w) => {
      const found = saved.find(
        (sw) => sw.foreign === w.foreign && sw.translation === w.translation,
      );
      if (found) w.level = found.level;
    });
  }

  state.words = words;
  state.category = catName;
  state.setName = `${catName} / ${setId}. Ünite`;
  state.streak = 0;
  state.totalCorrect = 0;
  state.totalAnswered = 0;
  state.hintUsed = false;
  state._categoryId = categoryId;
  state._setId = setId;

  showScreen("gameScreen");
  document.getElementById("gameSetName").textContent = state.setName;
  pickRandomWord();
  updateProgress();
  updateStreakBadge();
  clearFeedback();
  focusInput();
}

function continueGame() {
  if (state.words.length > 0) {
    showScreen("gameScreen");
    pickRandomWord();
    updateProgress();
    focusInput();
  }
}

function pickRandomWord() {
  if (state.words.length === 0) return;

  const unmastered = state.words.filter((w) => w.level < 5);
  const pool = unmastered.length > 0 ? unmastered : state.words;

  const weights = pool.map((w) => Math.max(6 - w.level, 1));
  const totalWeight = weights.reduce((a, b) => a + b, 0);
  let rand = Math.random() * totalWeight;

  let chosen = pool[0];
  for (let i = 0; i < pool.length; i++) {
    rand -= weights[i];
    if (rand <= 0) {
      chosen = pool[i];
      break;
    }
  }

  const chosenIndex = state.words.indexOf(chosen);
  if (chosenIndex === state.currentIndex && state.words.length > 1) {
    const others = pool.filter(
      (w) => state.words.indexOf(w) !== state.currentIndex,
    );
    if (others.length > 0) {
      chosen = others[Math.floor(Math.random() * others.length)];
    }
  }

  state.currentIndex = state.words.indexOf(chosen);
  state.hintUsed = false;
  displayWord();
}

function displayWord() {
  const word = state.words[state.currentIndex];
  if (!word) return;

  const foreignEl = document.getElementById("wordForeign");
  foreignEl.textContent = word.foreign;
  foreignEl.style.animation = "none";
  foreignEl.offsetHeight;
  foreignEl.style.animation = "wordAppear 0.4s ease-out";

  const level = word.level;
  const stars = "*".repeat(level) + "-".repeat(5 - level);
  document.getElementById("levelStars").textContent = stars;
  document.getElementById("levelNum").textContent = level;

  const levelEl = document.getElementById("wordLevel");
  levelEl.className = `word-level level-${level}`;

  document.getElementById("answerInput").value = "";
  document.getElementById("answerInput").className = "answer-input";
}

function checkAnswer() {
  const input = document.getElementById("answerInput");
  const answer = input.value.trim();
  if (!answer) {
    input.focus();
    return;
  }

  const word = state.words[state.currentIndex];
  const data = JSON.parse(
    wasmCheckAnswer(answer, word.translation, word.foreign, word.level),
  );

  state.words[state.currentIndex].level = data.new_level;
  state.totalAnswered++;

  if (data.correct) {
    state.totalCorrect++;
    state.streak++;
    if (state.streak > state.bestStreak) state.bestStreak = state.streak;
    input.className = "answer-input correct";
    showFeedback(data.feedback, data.close_match ? "close" : "correct");

    if (state.streak >= 3) {
      showStreakAnimation();
    }
  } else {
    state.streak = 0;
    input.className = "answer-input wrong";
    showFeedback(data.feedback, data.close_match ? "close" : "wrong");
    shakeWordCard();
  }

  updateStreakBadge();
  updateProgress();
  saveProgress();

  const mastered = state.words.filter((w) => w.level >= 5).length;
  const allMastered = mastered === state.words.length && state.words.length > 0;

  if (allMastered) {
    setTimeout(() => {
      showEndScreen();
    }, 1200);
    return;
  }

  setTimeout(() => {
    pickRandomWord();
    focusInput();
  }, 1500);
}

function skipWord() {
  const word = state.words[state.currentIndex];
  showFeedback(`Geçildi. Cevap: "${word.translation}"`, "wrong");
  state.streak = 0;
  updateStreakBadge();

  setTimeout(() => {
    pickRandomWord();
    focusInput();
  }, 1500);
}

function showHint() {
  const word = state.words[state.currentIndex];
  const translation = word.translation;

  if (state.hintUsed) {
    const revealed = Math.ceil(translation.length * 0.6);
    const hint =
      translation.substring(0, revealed) +
      ".".repeat(translation.length - revealed);
    showFeedback(`İpucu: ${hint}`, "close");
  } else {
    const firstChar = translation.charAt(0);
    const hint = firstChar + ".".repeat(translation.length - 1);
    showFeedback(`İpucu: ${hint} (${translation.length} harf)`, "close");
    state.hintUsed = true;
  }
}

function updateProgress() {
  const mastered = state.words.filter((w) => w.level >= 5).length;
  const total = state.words.length;
  const pct = total > 0 ? (mastered / total) * 100 : 0;

  document.getElementById("progressLabel").textContent = `${mastered}/${total}`;
  document.getElementById("progressBar").style.width = `${pct}%`;
}

function saveProgress() {
  if (state._categoryId && state._setId) {
    const key = `${state._categoryId}_${state._setId}`;
    state.progress[key] = state.words.map((w) => ({
      foreign: w.foreign,
      translation: w.translation,
      level: w.level,
    }));
  }
  state.stats.bestStreak = Math.max(
    state.stats.bestStreak || 0,
    state.bestStreak,
  );
  state.stats.totalAnswered =
    (state.stats.totalAnswered || 0) + 1;
  saveLocalProgress();
  syncProgressToServer();
}

function showFeedback(message, type) {
  const container = document.getElementById("feedbackContainer");
  container.innerHTML = `<div class="feedback feedback-${type}">${message}</div>`;
}

function clearFeedback() {
  document.getElementById("feedbackContainer").innerHTML = "";
}

function shakeWordCard() {
  const card = document.getElementById("wordCard");
  card.style.animation = "none";
  card.offsetHeight;
  card.style.animation = "shake 0.5s ease-out";
}

function updateStreakBadge() {
  const badge = document.getElementById("streakBadge");
  if (state.streak >= 2) {
    badge.style.display = "inline-flex";
    document.getElementById("streakCount").textContent = state.streak;
    badge.style.animation = "none";
    badge.offsetHeight;
    badge.style.animation = "streakPulse 0.5s ease-out";
  } else {
    badge.style.display = "none";
  }
}

function showStreakAnimation() {
  const el = document.createElement("div");
  el.textContent = `${state.streak} seri!`;
  el.style.cssText = `
        position: fixed;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        font-size: 2rem;
        font-weight: 800;
        color: var(--yellow);
        pointer-events: none;
        z-index: 50;
        animation: streakFloat 1s ease-out forwards;
    `;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 1000);
}

function showEndScreen() {
  showScreen("endScreen");
  document.getElementById("endStats").textContent =
    `${state.words.length} kelime ezberlediniz! (Doğruluk: ${Math.round((state.totalCorrect / Math.max(state.totalAnswered, 1)) * 100)}%)`;
  launchConfetti();
}

function replayGame() {
  state.words.forEach((w) => (w.level = 1));
  state.streak = 0;
  state.totalCorrect = 0;
  state.totalAnswered = 0;
  saveProgress();

  showScreen("gameScreen");
  pickRandomWord();
  updateProgress();
  clearFeedback();
  focusInput();
}

function launchConfetti() {
  const container = document.getElementById("confettiContainer");
  container.innerHTML = "";
  const colors = [
    "#ef4444",
    "#eab308",
    "#22c55e",
    "#3b82f6",
    "#8b5cf6",
    "#ec4899",
    "#f97316",
  ];

  for (let i = 0; i < 80; i++) {
    const confetti = document.createElement("div");
    confetti.className = "confetti";
    const color = colors[Math.floor(Math.random() * colors.length)];
    const left = Math.random() * 100;
    const delay = Math.random() * 2;
    const duration = 2 + Math.random() * 3;
    const size = 6 + Math.random() * 10;
    const shape = Math.random() > 0.5 ? "50%" : "0";

    confetti.style.cssText = `
            left: ${left}%;
            background: ${color};
            width: ${size}px;
            height: ${size}px;
            border-radius: ${shape};
            animation-delay: ${delay}s;
            animation-duration: ${duration}s;
        `;
    container.appendChild(confetti);
  }

  setTimeout(() => (container.innerHTML = ""), 6000);
}

function showAddWords() {
  showScreen("addWordsScreen");
  state.customWords = [];
  renderCustomWordList();
}

function addWord() {
  const foreign = document.getElementById("addForeign").value.trim();
  const translation = document.getElementById("addTranslation").value.trim();

  if (!foreign || !translation) {
    showToast("Her iki alanı da doldurun!", "error");
    return;
  }

  state.customWords.push({ foreign, translation, level: 1 });
  document.getElementById("addForeign").value = "";
  document.getElementById("addTranslation").value = "";
  document.getElementById("addForeign").focus();

  renderCustomWordList();
  showToast("Kelime eklendi!", "success");
}

function deleteCustomWord(index) {
  state.customWords.splice(index, 1);
  renderCustomWordList();
}

function renderCustomWordList() {
  const list = document.getElementById("customWordList");
  const btn = document.getElementById("startCustomGameBtn");

  if (state.customWords.length === 0) {
    list.innerHTML = `
            <div class="empty-state">
                <div class="icon">+</div>
                <p>Henüz kelime eklenmedi. Ezberlemeye başlamak için kelime ekleyin!</p>
            </div>
        `;
    btn.style.display = "none";
    return;
  }

  btn.style.display = "inline-flex";
  list.innerHTML = "";
  state.customWords.forEach((w, i) => {
    const item = document.createElement("div");
    item.className = "word-list-item";
    const pair = document.createElement("div");
    pair.className = "word-pair";
    pair.innerHTML = `${w.foreign} = <span>${w.translation}</span>`;
    const del = document.createElement("button");
    del.className = "delete-btn";
    del.textContent = "×";
    del.onclick = () => deleteCustomWord(i);
    item.appendChild(pair);
    item.appendChild(del);
    list.appendChild(item);
  });
}

function startCustomGame() {
  if (state.customWords.length === 0) {
    showToast("En az 1 kelime ekleyin!", "error");
    return;
  }

  state.words = state.customWords.map((w) => ({ ...w }));
  state.category = "Özel";
  state.setName = "Özel Kelime Listesi";
  state.streak = 0;
  state.totalCorrect = 0;
  state.totalAnswered = 0;
  state._categoryId = "ozel";
  state._setId = "liste";

  showScreen("gameScreen");
  document.getElementById("gameSetName").textContent = "Özel Kelime Listesi";
  pickRandomWord();
  updateProgress();
  clearFeedback();
  focusInput();
}

function showToast(message, type = "success") {
  const container = document.getElementById("toastContainer");
  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;
  toast.textContent = message;
  container.appendChild(toast);
  setTimeout(() => toast.remove(), 3000);
}

function focusInput() {
  setTimeout(() => {
    const input = document.getElementById("answerInput");
    if (input) input.focus();
  }, 100);
}

document.addEventListener("keydown", (e) => {
  if (
    e.key === "Enter" &&
    document.getElementById("gameScreen").classList.contains("active")
  ) {
    const input = document.getElementById("answerInput");
    if (document.activeElement === input && input.value.trim()) {
      checkAnswer();
    }
  }

  if (
    e.key === "Tab" &&
    document.getElementById("gameScreen").classList.contains("active")
  ) {
    e.preventDefault();
    skipWord();
  }

  if (
    e.key === "Enter" &&
    document.getElementById("authScreen").classList.contains("active")
  ) {
    const passInput = document.getElementById("authPassword");
    if (document.activeElement === passInput && passInput.value) {
      doLogin();
    }
  }

  if (
    e.key === "Enter" &&
    document.getElementById("addWordsScreen").classList.contains("active")
  ) {
    const foreignInput = document.getElementById("addForeign");
    const transInput = document.getElementById("addTranslation");
    if (
      document.activeElement === transInput &&
      transInput.value.trim() &&
      foreignInput.value.trim()
    ) {
      addWord();
    } else if (document.activeElement === foreignInput) {
      transInput.focus();
    }
  }
});

window.doLogin = doLogin;
window.doRegister = doRegister;
window.skipAuth = skipAuth;
window.doLogout = doLogout;
window.showMenu = showMenu;
window.showCategories = showCategories;
window.showSets = showSets;
window.startGame = startGame;
window.continueGame = continueGame;
window.checkAnswer = checkAnswer;
window.skipWord = skipWord;
window.showHint = showHint;
window.replayGame = replayGame;
window.showAddWords = showAddWords;
window.addWord = addWord;
window.deleteCustomWord = deleteCustomWord;
window.startCustomGame = startCustomGame;

document.addEventListener("DOMContentLoaded", async () => {
  await initWasm();

  const savedAuth = sessionStorage.getItem("wortmeister_auth");
  if (savedAuth) {
    try {
      const parsed = JSON.parse(savedAuth);
      auth.username = parsed.username;
      auth.token = parsed.token;
      const info = document.getElementById("menuUserInfo");
      info.textContent = `Hoş geldin, ${parsed.username}!`;
      info.style.display = "block";
      document.getElementById("logoutBtn").style.display = "flex";
      state.progress = loadLocalProgress();
      showMenu();
      return;
    } catch (e) {
      sessionStorage.removeItem("wortmeister_auth");
    }
  }
  showScreen("authScreen");
});
