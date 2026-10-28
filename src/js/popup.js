// Clipboard popup: history list, pinned palette, search and keyboard navigation.
const $ = (id) => document.getElementById(id);

const state = {
  settings: null,
  entries: [],
  query: "",
  panel: "history",
  index: { history: 0, pinned: 0 },
  tagFilter: null,
};
const thumbs = new Map();

const els = {
  app: $("app"),
  search: $("search"),
  clearSearch: $("clear-search"),
  historyList: $("history-list"),
  pinnedList: $("pinned-list"),
  historyEmpty: $("history-empty"),
  historyEmptyText: $("history-empty-text"),
  pinnedEmpty: $("pinned-empty"),
  historyCount: $("history-count"),
  tagFilter: $("tag-filter"),
  tagMenu: $("tag-menu"),
};

const pinnedEnabled = () => state.settings?.pinned_enabled;
const pinnedOpen = () => pinnedEnabled() && !state.settings.pinned_collapsed;

function lists() {
  const history = pinnedEnabled() ? state.entries.filter((e) => !e.pinned) : state.entries;
  const pinned = state.entries.filter(
    (e) => e.pinned && (!state.tagFilter || e.tags.includes(state.tagFilter)),
  );
  return { history, pinned };
}

function currentEntry() {
  const list = lists()[state.panel];
  return list[state.index[state.panel]];
}

function timeAgo(ms) {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return `${Math.round(s / 3600)} h ago`;
  if (s < 86400 * 7) return `${Math.round(s / 86400)} d ago`;
  return new Date(ms).toLocaleDateString();
}

function tagById(id) {
  return state.settings.tags.find((t) => t.id === id);
}

function renderItem(entry, i, panel) {
  const selected = state.panel === panel && state.index[panel] === i;
  const li = document.createElement("li");
  li.className = "item" + (selected ? " selected" : "");
  li.dataset.id = entry.id;
  li.dataset.panel = panel;
  li.dataset.index = i;

  const content =
    entry.kind === "image"
      ? `<div class="content image"><img alt="" data-thumb="${entry.id}" /></div>`
      : `<div class="content">${escapeHtml(entry.preview)}</div>`;

  const info =
    entry.kind === "image"
      ? `${icon("image")}<span class="grow">${entry.width} × ${entry.height}</span>`
      : `<span class="grow">${timeAgo(entry.created)} · ${entry.chars.toLocaleString()} chars</span>`;

  const tags = entry.pinned
    ? entry.tags
        .map(tagById)
        .filter(Boolean)
        .map((t) => `<span class="tag-pill" style="background:${t.color}">${escapeHtml(t.name)}</span>`)
        .join("")
    : "";

  const idx = i < 9 ? `<span class="index" title="Ctrl+${i + 1}">${i + 1}</span>` : "";
  const pinBtn = pinnedEnabled() || entry.pinned
    ? `<button class="icon-btn stateful pin ${entry.pinned ? "pinned-on" : ""}" data-action="pin" title="${entry.pinned ? "Unpin" : "Pin"} (Ctrl+P)">${icon("pin")}</button>`
    : "";
  const tagBtn = entry.pinned && pinnedEnabled()
    ? `<button class="icon-btn stateful" data-action="tags" title="Tags">${icon("label")}</button>`
    : "";

  li.innerHTML = `${content}
    <div class="meta">${idx}${info}${tags}</div>
    <div class="actions">${tagBtn}${pinBtn}<button class="icon-btn stateful del" data-action="delete" title="Delete (Del)">${icon("delete")}</button></div>`;
  return li;
}

function render() {
  const { history, pinned } = lists();
  for (const panel of ["history", "pinned"]) {
    const len = (panel === "history" ? history : pinned).length;
    state.index[panel] = Math.min(state.index[panel], Math.max(0, len - 1));
  }
  if (!pinnedOpen()) state.panel = "history";
  $("pinned-count").textContent = state.entries.some((e) => e.pinned) ? String(state.entries.filter((e) => e.pinned).length) : "";

  els.historyList.replaceChildren(...history.map((e, i) => renderItem(e, i, "history")));
  els.pinnedList.replaceChildren(...pinned.map((e, i) => renderItem(e, i, "pinned")));
  els.historyEmpty.hidden = history.length > 0;
  els.historyEmptyText.textContent = state.query
    ? "Nothing matches your search."
    : "Copy something and it will show up here.";
  els.pinnedEmpty.hidden = pinned.length > 0;
  els.historyCount.textContent = history.length ? String(history.length) : "";
  renderTagFilter();
  loadThumbs();
  scrollToSelection();
}

function renderTagFilter() {
  const tags = state.settings.tags;
  els.tagFilter.hidden = tags.length === 0;
  const chip = (id, label, color) =>
    `<button class="chip stateful ${state.tagFilter === id ? "selected" : ""}" data-tag="${id ?? ""}">` +
    (color ? `<span class="dot" style="background:${color}"></span>` : "") +
    `${escapeHtml(label)}</button>`;
  els.tagFilter.innerHTML = chip(null, "All") + tags.map((t) => chip(t.id, t.name, t.color)).join("");
}

async function loadThumbs() {
  for (const img of document.querySelectorAll("img[data-thumb]")) {
    const id = Number(img.dataset.thumb);
    if (!thumbs.has(id)) {
      try {
        thumbs.set(id, await invoke("get_image", { id, thumb: true }));
      } catch {
        continue;
      }
    }
    img.src = thumbs.get(id);
  }
}

function scrollToSelection() {
  const list = state.panel === "history" ? els.historyList : els.pinnedList;
  list.querySelector(".item.selected")?.scrollIntoView({ block: "nearest" });
}

async function reload() {
  state.entries = await invoke("get_entries", { query: state.query });
  render();
}

async function loadSettings(settings) {
  state.settings = settings ?? (await invoke("get_settings"));
  applyTheme(state.settings);
  els.app.classList.toggle("no-pinned", !state.settings.pinned_enabled);
  els.app.classList.toggle("pinned-left", state.settings.pinned_side === "left");
  applyCollapsed();
  if (state.tagFilter && !tagById(state.tagFilter)) state.tagFilter = null;
}

// Pinned palette collapse (state is remembered by the backend)
function applyCollapsed() {
  const collapsed = !!state.settings.pinned_collapsed;
  const left = state.settings.pinned_side === "left";
  els.app.classList.toggle("pinned-collapsed", collapsed);
  // Chevrons point in the direction the palette moves.
  $("rail-chevron").innerHTML = icon(left ? "chevronRight" : "chevronLeft");
  $("collapse-pinned").innerHTML = icon(left ? "chevronLeft" : "chevronRight");
}

function setPinnedCollapsed(collapsed) {
  if (!collapsed && !pinnedEnabled()) return;
  state.settings.pinned_collapsed = collapsed;
  if (collapsed) state.panel = "history";
  closeTagMenu();
  applyCollapsed();
  render();
  invoke("set_pinned_collapsed", { collapsed });
}

$("pinned-rail").addEventListener("click", () => setPinnedCollapsed(false));
$("collapse-pinned").addEventListener("click", () => setPinnedCollapsed(true));

// Actions
async function paste(entry) {
  if (!entry) return;
  try {
    await invoke("paste_entry", { id: entry.id });
  } catch (e) {
    snackbar(String(e));
  }
}
async function togglePin(entry) {
  if (entry) await invoke("toggle_pin", { id: entry.id });
}
async function remove(entry) {
  if (!entry) return;
  thumbs.delete(entry.id);
  await invoke("delete_entry", { id: entry.id });
}

function select(panel, index) {
  state.panel = panel;
  state.index[panel] = index;
  render();
}

// Tag menu
let menuEntry = null;
function openTagMenu(entry, anchor) {
  menuEntry = entry;
  const tags = state.settings.tags;
  els.tagMenu.innerHTML =
    tags
      .map(
        (t) => `<button class="stateful ${entry.tags.includes(t.id) ? "on" : ""}" data-tag="${t.id}">
          <span class="dot" style="background:${t.color}"></span><span class="name">${escapeHtml(t.name)}</span>
          <span class="check">${icon("check")}</span></button>`,
      )
      .join("") +
    (tags.length ? "<hr />" : "") +
    `<button class="stateful muted" data-manage>${icon("settings")}<span class="name">Manage tags…</span></button>`;
  els.tagMenu.hidden = false;
  const r = anchor.getBoundingClientRect();
  const m = els.tagMenu.getBoundingClientRect();
  const left = Math.min(r.right - m.width, window.innerWidth - m.width - 8);
  const top = r.bottom + m.height + 8 > window.innerHeight ? r.top - m.height - 4 : r.bottom + 4;
  els.tagMenu.style.left = `${Math.max(8, left)}px`;
  els.tagMenu.style.top = `${Math.max(8, top)}px`;
}
function closeTagMenu() {
  els.tagMenu.hidden = true;
  menuEntry = null;
}

els.tagMenu.addEventListener("click", async (ev) => {
  const btn = ev.target.closest("button");
  if (!btn || !menuEntry) return;
  if (btn.hasAttribute("data-manage")) {
    closeTagMenu();
    invoke("open_settings", { section: "tags" });
    return;
  }
  const id = btn.dataset.tag;
  const tags = menuEntry.tags.includes(id) ? menuEntry.tags.filter((t) => t !== id) : [...menuEntry.tags, id];
  menuEntry.tags = tags;
  btn.classList.toggle("on");
  await invoke("set_entry_tags", { id: menuEntry.id, tags });
});

// Mouse
for (const list of [els.historyList, els.pinnedList]) {
  list.addEventListener("click", (ev) => {
    const item = ev.target.closest(".item");
    if (!item) return;
    const entry = state.entries.find((e) => e.id === Number(item.dataset.id));
    const action = ev.target.closest("[data-action]")?.dataset.action;
    if (action === "pin") return togglePin(entry);
    if (action === "delete") return remove(entry);
    if (action === "tags") return openTagMenu(entry, ev.target.closest("button"));
    paste(entry);
  });
}

els.tagFilter.addEventListener("click", (ev) => {
  const chip = ev.target.closest(".chip");
  if (!chip) return;
  state.tagFilter = chip.dataset.tag || null;
  state.index.pinned = 0;
  render();
});

document.addEventListener("mousedown", (ev) => {
  if (!els.tagMenu.hidden && !els.tagMenu.contains(ev.target) && !ev.target.closest("[data-action=tags]")) {
    closeTagMenu();
  }
});

$("open-settings").addEventListener("click", () => invoke("open_settings", { section: null }));
$("manage-tags").addEventListener("click", () => invoke("open_settings", { section: "tags" }));
$("clear-all").addEventListener("click", async () => {
  thumbs.clear();
  await invoke("clear_history");
  snackbar("History cleared");
});
els.clearSearch.addEventListener("click", () => {
  els.search.value = "";
  onSearch();
  els.search.focus();
});

function onSearch() {
  state.query = els.search.value;
  els.clearSearch.hidden = !state.query;
  state.index = { history: 0, pinned: 0 };
  reload();
}
els.search.addEventListener("input", onSearch);

// Keyboard
document.addEventListener("keydown", (ev) => {
  const { history, pinned } = lists();
  const list = state.panel === "history" ? history : pinned;
  const ctrl = ev.ctrlKey || ev.metaKey;

  switch (ev.key) {
    case "Escape":
      ev.preventDefault();
      if (!els.tagMenu.hidden) return closeTagMenu();
      if (els.search.value) {
        els.search.value = "";
        return onSearch();
      }
      return invoke("hide_popup");
    case "ArrowDown":
    case "ArrowUp": {
      ev.preventDefault();
      if (!list.length) return;
      const delta = ev.key === "ArrowDown" ? 1 : -1;
      const next = (state.index[state.panel] + delta + list.length) % list.length;
      return select(state.panel, next);
    }
    case "Tab": {
      ev.preventDefault();
      if (!pinnedEnabled()) return;
      if (state.settings.pinned_collapsed) {
        setPinnedCollapsed(false);
        return select("pinned", state.index.pinned);
      }
      const other = state.panel === "history" ? "pinned" : "history";
      return select(other, state.index[other]);
    }
    case "Enter":
      ev.preventDefault();
      return paste(currentEntry());
    case "Delete":
      if (!els.search.value || ctrl) {
        ev.preventDefault();
        remove(currentEntry());
      }
      return;
  }

  if (ctrl && ev.key.toLowerCase() === "p") {
    ev.preventDefault();
    if (pinnedEnabled() || currentEntry()?.pinned) togglePin(currentEntry());
    return;
  }
  if (ctrl && /^[1-9]$/.test(ev.key)) {
    ev.preventDefault();
    paste(list[Number(ev.key) - 1]);
    return;
  }
  if (document.activeElement !== els.search && ev.key.length === 1 && !ctrl && !ev.altKey) {
    els.search.focus();
  }
});

// Events from the backend
listen("popup-shown", async () => {
  closeTagMenu();
  els.search.value = "";
  state.query = "";
  els.clearSearch.hidden = true;
  state.panel = "history";
  state.index = { history: 0, pinned: 0 };
  await reload();
  els.search.focus();
});
listen("history-changed", () => reload());
listen("settings-changed", async (ev) => {
  await loadSettings(ev.payload);
  render();
});
darkQuery.addEventListener("change", () => state.settings && applyTheme(state.settings));
document.addEventListener("contextmenu", (ev) => ev.preventDefault());
// Focus comes back here after the popup was dragged; keep typing going to the search box.
window.addEventListener("focus", () => {
  if (els.tagMenu.hidden) els.search.focus();
});

(async function init() {
  hydrateIcons();
  await loadSettings();
  await reload();
})();
