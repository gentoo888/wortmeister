const API = "https://wortmeister.onrender.com";
let state = {
  sessionId: null,
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
};

function showScreen(id) {
  document
    .querySelectorAll(".screen")
    .forEach((s) => s.classList.remove("active"));
  const screen = document.getElementById(id);
  screen.classList.add("active");
  screen.style.animation = "none";
  screen.offsetHeight; // reflow
  screen.style.animation = "";
}

function showMenu() {
  showScreen("menuScreen");
  updateContinueButton();
}

function updateContinueButton() {
  const btn = document.getElementById("continueBtn");
  if (state.sessionId && state.words.length > 0) {
    btn.style.display = "flex";
    btn.textContent = `Devam Et (${state.setName})`;
  } else {
    btn.style.display = "none";
  }
}

async function showCategories() {
  showScreen("categoryScreen");
  const grid = document.getElementById("categoryGrid");
  grid.innerHTML = '<div class="loading-spinner"></div>';

  try {
    const res = await fetch(`${API}/api/categories`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const categories = await res.json();

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
                <div class="card-info">${cat.set_count} unite</div>
            `;
      grid.appendChild(card);
    });
  } catch (e) {
    grid.innerHTML = '<p style="color:var(--red)">Kategoriler yuklenemedi.</p>';
    console.error(e);
  }
}

async function showSets(categoryId, categoryName) {
  showScreen("setScreen");
  document.getElementById("setScreenTitle").textContent = `${categoryName}`;
  document.getElementById("setScreenSub").textContent =
    "Calismak istedigin uniteyi sec";

  const grid = document.getElementById("setGrid");
  grid.innerHTML = '<div class="loading-spinner"></div>';

  try {
    const res = await fetch(`${API}/api/categories/${categoryId}/sets`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const sets = await res.json();

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
  } catch (e) {
    grid.innerHTML = '<p style="color:var(--red)">Uniteler yuklenemedi.</p>';
    console.error(e);
  }
}

async function startGame(categoryId, setId) {
  try {
    const res = await fetch(`${API}/api/game/start`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ category_id: categoryId, set_id: setId }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    const savedKey = `progress_${categoryId}_${setId}`;
    const saved = localStorage.getItem(savedKey);
    if (saved) {
      try {
        const savedWords = JSON.parse(saved);
        data.words.forEach((w) => {
          const found = savedWords.find(
            (sw) =>
              sw.foreign === w.foreign && sw.translation === w.translation,
          );
          if (found) w.level = found.level;
        });
      } catch (e) {
        /* ignore parse errors */
      }
    }

    state.sessionId = data.session_id;
    state.words = data.words;
    state.category = data.category;
    state.setName = data.set_name;
    state.streak = 0;
    state.totalCorrect = 0;
    state.totalAnswered = 0;
    state.hintUsed = false;
    state._categoryId = categoryId;
    state._setId = setId;

    showScreen("gameScreen");
    document.getElementById("gameSetName").textContent = data.set_name;
    pickRandomWord();
    updateProgress();
    updateStreakBadge();
    clearFeedback();
    focusInput();
  } catch (e) {
    showToast("Oyun baslatilamadi!", "error");
    console.error(e);
  }
}

function continueGame() {
  if (state.sessionId && state.words.length > 0) {
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

async function checkAnswer() {
  const input = document.getElementById("answerInput");
  const answer = input.value.trim();
  if (!answer) {
    input.focus();
    return;
  }

  try {
    const res = await fetch(`${API}/api/game/check`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        session_id: state.sessionId,
        word_index: state.currentIndex,
        answer: answer,
      }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

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

    if (data.all_mastered) {
      setTimeout(() => {
        showEndScreen();
      }, 1200);
      return;
    }

    setTimeout(() => {
      pickRandomWord();
      focusInput();
    }, 1500);
  } catch (e) {
    showToast("Baglanti hatasi!", "error");
    console.error(e);
  }
}

function skipWord() {
  const word = state.words[state.currentIndex];
  showFeedback(`Gecildi. Cevap: "${word.translation}"`, "wrong");
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
    // Show more hint (im not sure if this is more than needed but nevermind)
    const revealed = Math.ceil(translation.length * 0.6);
    const hint =
      translation.substring(0, revealed) +
      ".".repeat(translation.length - revealed);
    showFeedback(`Ipucu: ${hint}`, "close");
  } else {
    // Show first letter hint
    const firstChar = translation.charAt(0);
    const hint = firstChar + ".".repeat(translation.length - 1);
    showFeedback(`Ipucu: ${hint} (${translation.length} harf)`, "close");
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
    const key = `progress_${state._categoryId}_${state._setId}`;
    localStorage.setItem(key, JSON.stringify(state.words));
  }
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

const shakeStyle = document.createElement("style");
shakeStyle.textContent = `
    @keyframes shake {
        0%, 100% { transform: translateX(0); }
        10%, 50%, 90% { transform: translateX(-6px); }
        30%, 70% { transform: translateX(6px); }
    }
`;
document.head.appendChild(shakeStyle);

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

const streakFloatStyle = document.createElement("style");
streakFloatStyle.textContent = `
    @keyframes streakFloat {
        0% { opacity: 1; transform: translate(-50%, -50%) scale(0.5); }
        50% { opacity: 1; transform: translate(-50%, -70%) scale(1.2); }
        100% { opacity: 0; transform: translate(-50%, -90%) scale(1); }
    }
`;
document.head.appendChild(streakFloatStyle);

function showEndScreen() {
  showScreen("endScreen");
  document.getElementById("endStats").textContent =
    `${state.words.length} kelime ezberlediniz! (Dogruluk: ${Math.round((state.totalCorrect / Math.max(state.totalAnswered, 1)) * 100)}%)`;
  launchConfetti();
}

async function replayGame() {
  state.words.forEach((w) => (w.level = 1));
  state.streak = 0;
  state.totalCorrect = 0;
  state.totalAnswered = 0;
  saveProgress();

  try {
    await fetch(`${API}/api/game/reset`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: state.sessionId }),
    });
  } catch (e) {
    /* ignore */
  }

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
    showToast("Her iki alani da doldurun!", "error");
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
                <p>Henuz kelime eklenmedi. Ezberlemeye baslamak icin kelime ekleyin!</p>
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
    item.innerHTML = `
            <div class="word-pair">${w.foreign} = <span>${w.translation}</span></div>
            <button class="delete-btn" onclick="deleteCustomWord(${i})">×</button>
        `;
    list.appendChild(item);
  });
}

async function startCustomGame() {
  if (state.customWords.length === 0) {
    showToast("En az 1 kelime ekleyin!", "error");
    return;
  }

  try {
    const res = await fetch(`${API}/api/game/custom-session`, {
      method: "POST",
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    state.sessionId = data.session_id;
    state.words = [...state.customWords];
    state.category = "Ozel";
    state.setName = "Ozel Kelime Listesi";
    state.streak = 0;
    state.totalCorrect = 0;
    state.totalAnswered = 0;

    for (const w of state.customWords) {
      await fetch(`${API}/api/game/add-word`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          session_id: state.session_id,
          foreign: w.foreign,
          translation: w.translation,
        }),
      });
    }

    showScreen("gameScreen");
    document.getElementById("gameSetName").textContent = "Ozel Kelime Listesi";
    pickRandomWord();
    updateProgress();
    clearFeedback();
    focusInput();
  } catch (e) {
    showToast("Oyun baslatilamadi!", "error");
    console.error(e);
  }
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
  // Enter to check answer in game
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

document.addEventListener("DOMContentLoaded", () => {
  updateContinueButton();
});
