// ScrapForge site — static page + live data from the GitHub API.
// No frameworks, no trackers, no external fonts.

const REPO = "vanyachickenganidanya-lgtm/test";
const API = "https://api.github.com/repos/" + REPO;

/* ------------------------------------------------------------ features */

const FEATURES = [
  { t: "Процедурный мир", d: "Рельеф, вода, туман и биомы считаются формулами из сида. Ни одной карты высот и ни одной текстуры на диске." },
  { t: "Строительство по вокселям", d: "Кубы, плиты, клинья, цилиндры, сферы и рамы. Ставьте, удаляйте, красьте в 14 цветов, поворачивайте." },
  { t: "Отмена и повтор", d: "Ctrl+Z / Ctrl+Y с историей правок — того, чего больше всего просят в креативных песочницах." },
  { t: "Инструмент «подъём»", d: "Берёт всю связную постройку целиком: перемещайте, вращайте клавишей R и ставьте обратно в сетку." },
  { t: "Копирование и вставка", d: "Ctrl+C снимает чертёж с постройки, Ctrl+V ставит копию с учётом текущего поворота." },
  { t: "Своя физика контрапций", d: "Подвеска на рейкастах, контактные точки по блокам, инерция по габаритам, сильное демпфирование — машинки не трясутся и не разваливаются." },
  { t: "33 детали", d: "Колёса трёх типов, сиденье, двигатели, ракетные ускорители, пропеллер, гироскоп, баллон, балластная цистерна, поршень, мотор-подшипник, свет и картофельная пушка." },
  { t: "Логика и автоматика", d: "Переключатели, кнопки, датчики, И/ИЛИ/Исключающее ИЛИ/НЕ, таймер, счётчик, ячейка памяти и инструмент «связь» для проводки." },
  { t: "Гироскоп и пропеллер", d: "То, чего игрокам не хватало больше всего: стабилизация полёта и воздушный движок вместо «глитча подвески»." },
  { t: "Плавучесть", d: "Балластные цистерны и вода, в которой можно тонуть и всплывать, а не просто декоративный слой." },
  { t: "Сохранения в JSON", d: "Мир — обычный текстовый файл. Его можно почитать, поправить руками и поделиться им как чертежом." },
  { t: "Настройки производительности", d: "Дальность прорисовки, тени, FOV и чувствительность меняются на ходу. Мерж блоков в один меш на секцию 16³." },
];

/* ------------------------------------------------------------- compare */

const COMPARE = [
  ["Вес игры", "Десятки ГБ установки", "~10 МБ одним бинарником, ~15 МБ веб-сборка"],
  ["Платформы", "Только Windows (Linux — через Proton)", "Windows, Linux, macOS и браузер (WebAssembly)"],
  ["Запуск", "Требуется Steam и онлайн-проверка", "Офлайн-бинарник или просто открытая веб-страница"],
  ["Исходный код", "Закрытый", "Открыт (MIT / Apache-2.0), можно форкать и патчить"],
  ["Текстуры", "Сотни МБ ассетов", "0 файлов: вся геометрия и все цвета генерируются кодом"],
  ["Отмена действий", "Нет", "Ctrl+Z / Ctrl+Y с историей"],
  ["Физика", "Bullet: джиттер, «глитч подвески», разваливающиеся сборки", "Свой сильно демпфированный решатель: подвеска на рейкастах"],
  ["Гироскоп", "Нет (приходится колхозить)", "Есть, отдельной деталью"],
  ["Пропеллер / авиация", "Нет, только ускорители", "Пропеллер с падением тяги на скорости + баллон"],
  ["Логика", "Только булевы сигналы", "Булевы + таймер, счётчик, ячейка памяти, настраиваемый датчик"],
  ["Чертежи", "Бинарные блупринты", "Обычный JSON: копировать, править, шарить"],
  ["Моддинг", "Ограниченные инструменты", "Открытый код + мир в JSON + свои детали правкой таблицы"],
  ["Мультиплеер", "Есть, но с жалобами на лаги и рассинхрон", "Пока нет — в планах после стабилизации физики"],
  ["Цена", "Платная + DLC", "Бесплатно и с открытым кодом"],
];

/* ----------------------------------------------------------- changelog */

const CHANGELOG = [
  {
    v: "0.1.0",
    date: "первый публичный релиз",
    items: [
      "Процедурный мир: рельеф на шуме, вода, туман, тени, стриминг чанков вокруг игрока.",
      "Воксельное строительство: 7 строительных форм, 14 цветов, поворот, постановка по граням.",
      "Инструменты: строительство, удаление, покраска, связь логики, подъём целой постройки.",
      "История правок (Ctrl+Z / Ctrl+Y), копирование и вставка чертежей (Ctrl+C / Ctrl+V).",
      "33 детали: колёса, сиденье, двигатели, ускорители, пропеллер, гироскоп, баллон, цистерна, поршень, мотор, свет, пушка.",
      "Своя физика контрапций: масса и инерция по блокам, подвеска на рейкастах, контактные точки, плавучесть.",
      "Логическая сеть: переключатели, кнопки, датчики, И/ИЛИ/XOR/НЕ, таймер, счётчик, память, проводка инструментом «связь».",
      "HUD, меню деталей (B), меню паузы (Esc), настройки дальности, теней, FOV и чувствительности.",
      "Сохранение и загрузка мира в JSON (F5 / F9) в десктопной сборке.",
      "Веб-сборка на WebAssembly: играть можно прямо на странице проекта.",
    ],
  },
];

/* ------------------------------------------------------------ controls */

const CONTROLS = [
  { t: "Ходьба и полёт", k: [["W A S D", "движение"], ["Space", "прыжок / вверх"], ["Shift", "спринт / ускорение"], ["Ctrl", "вниз в полёте"], ["F", "режим полёта"]] },
  { t: "Строительство", k: [["ЛКМ", "поставить"], ["ПКМ", "удалить"], ["СКМ", "взять образец"], ["R", "повернуть деталь"], ["1…9, 0", "слот панели"], ["B", "меню деталей"]] },
  { t: "Контрапции", k: [["E", "сесть / щёлкнуть переключатель"], ["Q", "выйти из сиденья"], ["G", "взять или бросить постройку"], ["Колесо", "сменить слот"]] },
  { t: "Логика и мир", k: [["C", "инструмент связи (проводка)"], ["Ctrl+Z / Ctrl+Y", "отмена / повтор"], ["Ctrl+C / Ctrl+V", "чертёж: копия и вставка"], ["F5 / F9", "сохранить / загрузить"], ["Esc", "пауза и настройки"]] },
];

/* ------------------------------------------------------------- helpers */

const el = (tag, cls, html) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (html !== undefined) n.innerHTML = html;
  return n;
};

const fmtSize = (bytes) => {
  if (!bytes && bytes !== 0) return "";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1) return mb.toFixed(1) + " МБ";
  return (bytes / 1024).toFixed(0) + " КБ";
};

const fmtDate = (iso) => {
  const d = new Date(iso);
  return d.toLocaleDateString("ru-RU", { year: "numeric", month: "long", day: "numeric" });
};

/* --------------------------------------------------------------- render */

function renderFeatures() {
  const grid = document.getElementById("feature-grid");
  FEATURES.forEach((f) => {
    const card = el("div", "card");
    card.appendChild(el("h3", null, f.t));
    card.appendChild(el("p", null, f.d));
    grid.appendChild(card);
  });
}

function renderCompare() {
  const body = document.getElementById("compare-body");
  COMPARE.forEach((row) => {
    const tr = el("tr");
    tr.appendChild(el("td", null, row[0]));
    tr.appendChild(el("td", null, row[1]));
    tr.appendChild(el("td", null, row[2]));
    body.appendChild(tr);
  });
}

function renderChangelog() {
  const box = document.getElementById("changelog-list");
  CHANGELOG.forEach((r) => {
    const div = el("div", "release");
    const h = el("h3", null, "v" + r.v + '<span class="date">' + r.date + "</span>");
    div.appendChild(h);
    const ul = el("ul");
    r.items.forEach((i) => ul.appendChild(el("li", null, i)));
    div.appendChild(ul);
    box.appendChild(div);
  });
}

function renderControls() {
  const grid = document.getElementById("controls-grid");
  CONTROLS.forEach((group) => {
    const card = el("div", "card");
    card.appendChild(el("h3", null, group.t));
    const table = el("table", "compare");
    const tbody = el("tbody");
    group.k.forEach(([key, desc]) => {
      const tr = el("tr");
      tr.appendChild(el("td", null, "<kbd>" + key + "</kbd>"));
      tr.appendChild(el("td", null, desc));
      tbody.appendChild(tr);
    });
    table.appendChild(tbody);
    card.appendChild(table);
    grid.appendChild(card);
  });
}

async function renderReleases() {
  const box = document.getElementById("release-box");
  try {
    const res = await fetch(API + "/releases?per_page=5");
    if (!res.ok) throw new Error("HTTP " + res.status);
    const releases = await res.json();
    if (!releases.length) {
      box.innerHTML =
        '<p>Релизов пока нет. Сборки появятся здесь после первого тега <code>v0.1.0</code>. ' +
        'Пока можно собрать из исходников: <code>cargo build --release</code>.</p>';
      return;
    }
    box.innerHTML = "";
    releases.forEach((rel, idx) => {
      const div = el("div");
      if (idx > 0) div.style.marginTop = "22px";
      const head = el("h3", null,
        rel.name || rel.tag_name +
        '<span class="date">' + fmtDate(rel.published_at) + "</span>");
      div.appendChild(head);
      if (rel.body) {
        const body = el("p", null, rel.body.split("\n").slice(0, 4).join("<br>"));
        body.style.color = "var(--muted)";
        body.style.fontSize = "14px";
        div.appendChild(body);
      }
      (rel.assets || []).forEach((a) => {
        const row = el("div", "asset");
        row.appendChild(el("span", "name", a.name));
        row.appendChild(el("span", "meta",
          fmtSize(a.size) + " · скачиваний: " + a.download_count));
        const link = el("a", "dl", "⬇ скачать");
        link.href = a.browser_download_url;
        row.appendChild(link);
        div.appendChild(row);
      });
      box.appendChild(div);
    });
  } catch (e) {
    box.innerHTML =
      "<p>Не удалось получить список релизов напрямую (" + e.message + "). " +
      'Откройте <a href="https://github.com/' + REPO + '/releases">страницу релизов</a>.</p>';
  }
}

async function checkGameBuild() {
  const note = document.getElementById("player-note");
  const btn = document.getElementById("play-btn");
  try {
    const res = await fetch("game/index.html", { method: "HEAD" });
    if (res.ok) {
      note.innerHTML =
        "Загрузка веб-сборки занимает 10–30 секунд (WASM ~15 МБ). " +
        "Управление: мышь — обзор, <kbd>WASD</kbd> — ходьба, <kbd>LMB</kbd> — поставить блок, " +
        "<kbd>E</kbd> — сесть в технику, <kbd>F</kbd> — полёт.";
    } else {
      throw new Error("no build");
    }
  } catch (e) {
    note.innerHTML =
      "Веб-сборка ещё не опубликована в этой ветке. Собрать её можно одной командой: " +
      "<code>cargo install trunk &amp;&amp; trunk build --release</code> — либо скачать готовый бинарник ниже.";
    if (btn) btn.href = "#downloads";
  }
}

document.querySelectorAll("button.copy").forEach((btn) => {
  btn.addEventListener("click", () => {
    const target = document.querySelector(btn.dataset.copy);
    if (!target) return;
    navigator.clipboard.writeText(target.textContent.trim());
    const old = btn.textContent;
    btn.textContent = "готово";
    setTimeout(() => (btn.textContent = old), 1400);
  });
});

renderFeatures();
renderCompare();
renderChangelog();
renderControls();
renderReleases();
checkGameBuild();
