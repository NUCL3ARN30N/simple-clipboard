// Settings window: every change is saved immediately (debounced).
const $ = (id) => document.getElementById(id);

const ACCENTS = ["#6750A4", "#3F51B5", "#0061A4", "#006A6A", "#386A20", "#B3261E", "#8B5000", "#984061"];
const WIN_V = "Super+V";

let settings = null;
let info = null;
let saveTimer = null;
let latestRelease = null;

function scheduleSave(delay = 250) {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, delay);
}

async function save() {
  saveTimer = null;
  try {
    settings = await invoke("save_settings", { settings });
  } catch (e) {
    snackbar(String(e));
    settings = await invoke("get_settings");
  }
  render();
}

function update(key, value, delay) {
  settings[key] = value;
  applyTheme(settings);
  renderDependent();
  scheduleSave(delay);
}

// Rendering
function render() {
  applyTheme(settings);
  document.querySelectorAll("input[type=checkbox][data-key]").forEach((el) => (el.checked = !!settings[el.dataset.key]));
  document.querySelectorAll(".segmented[data-key]").forEach((seg) => {
    seg.querySelectorAll("button").forEach((b) => b.classList.toggle("selected", b.dataset.value === settings[seg.dataset.key]));
  });
  $("reset-position").hidden = !settings.remember_position || !settings.popup_position;
  $("max-items").value = settings.max_items;
  $("max-items-range").value = settings.max_items;
  renderAccents();
  renderHotkey();
  renderTags();
  renderDependent();
}

function renderDependent() {
  document.querySelectorAll("[data-requires]").forEach((el) => {
    el.dataset.disabled = String(!settings[el.dataset.requires]);
  });
}

function renderAccents() {
  const current = settings.accent.toLowerCase();
  const isCustom = !ACCENTS.some((c) => c.toLowerCase() === current);
  $("accent-swatches").innerHTML =
    ACCENTS.map(
      (c) => `<button class="swatch ${c.toLowerCase() === current ? "selected" : ""}" style="background:${c}" data-color="${c}" title="${c}">${icon("check")}</button>`,
    ).join("") +
    `<label class="swatch custom ${isCustom ? "selected" : ""}" title="Custom color">${icon("check")}<input type="color" value="${settings.accent}" /></label>`;
}

function isWinV(hotkey) {
  return ["super+v", "win+v", "meta+v"].includes(hotkey.replace(/\s/g, "").toLowerCase());
}

function prettyHotkey(hotkey) {
  if (!hotkey) return "None";
  const win = info.platform === "windows";
  return hotkey
    .split("+")
    .map((p) => (/^(super|meta|win)$/i.test(p) ? (win ? "Win" : "Super") : p))
    .join(" + ");
}

function renderHotkey() {
  const windows = info.platform === "windows";
  $("hotkey-label").textContent = recording ? "Press keys…" : prettyHotkey(settings.hotkey);
  $("hotkey-btn").classList.toggle("recording", recording);
  $("win-v-row").hidden = !windows;
  $("win-v").checked = isWinV(settings.hotkey);
  $("shortcut-note").textContent = windows
    ? isWinV(settings.hotkey)
      ? "Replaces the built-in Windows clipboard history."
      : "Click to record a new shortcut. Esc cancels, Backspace clears."
    : "Click to record a new shortcut. On Wayland, bind the command \"simple-clipboard\" in your desktop's keyboard settings instead.";
  renderNative();
}

function renderTags() {
  const list = $("tag-list");
  if (!settings.tags.length) {
    list.innerHTML = `<li class="empty">No tags yet.</li>`;
    return;
  }
  list.innerHTML = settings.tags
    .map(
      (t, i) => `<li data-index="${i}">
        <label class="color" style="background:${t.color}" title="Change color"><input type="color" value="${t.color}" data-field="color" /></label>
        <input class="field" type="text" value="${escapeHtml(t.name)}" data-field="name" placeholder="Tag name" maxlength="32" />
        <button class="icon-btn stateful" data-remove title="Delete tag">${icon("delete")}</button>
      </li>`,
    )
    .join("");
}

// Inputs
document.querySelectorAll("input[type=checkbox][data-key]").forEach((el) =>
  el.addEventListener("change", () => update(el.dataset.key, el.checked)),
);
document.querySelectorAll(".segmented[data-key]").forEach((seg) =>
  seg.addEventListener("click", (ev) => {
    const b = ev.target.closest("button");
    if (!b) return;
    seg.querySelectorAll("button").forEach((x) => x.classList.toggle("selected", x === b));
    update(seg.dataset.key, b.dataset.value);
  }),
);

$("max-items-range").addEventListener("input", (ev) => {
  $("max-items").value = ev.target.value;
  update("max_items", Number(ev.target.value), 500);
});
$("max-items").addEventListener("change", (ev) => {
  const v = Math.min(1000, Math.max(1, Number(ev.target.value) || 50));
  ev.target.value = v;
  $("max-items-range").value = v;
  update("max_items", v);
});

$("accent-swatches").addEventListener("click", (ev) => {
  const b = ev.target.closest("button.swatch");
  if (!b) return;
  update("accent", b.dataset.color);
  renderAccents();
});
$("accent-swatches").addEventListener("input", (ev) => {
  if (ev.target.type === "color") update("accent", ev.target.value, 400);
});
$("accent-swatches").addEventListener("change", (ev) => {
  if (ev.target.type === "color") renderAccents();
});

// Hotkey recorder
let recording = false;
let previousHotkey = null;

$("hotkey-btn").addEventListener("click", () => {
  recording = !recording;
  renderHotkey();
});

$("win-v").addEventListener("change", (ev) => {
  if (ev.target.checked) {
    previousHotkey = settings.hotkey;
    update("hotkey", WIN_V);
  } else {
    update("hotkey", previousHotkey && !isWinV(previousHotkey) ? previousHotkey : "Ctrl+Shift+V");
  }
  renderHotkey();
});

// Built-in Windows Win+V (Explorer's DisabledHotkeys)
let nativeDisabled = false;
let nativeConfirm = false;
let nativeBusy = false;

function renderNative() {
  const row = $("native-row");
  row.hidden = info.platform !== "windows";
  if (row.hidden) return;
  const btn = $("native-btn");
  const conflict = isWinV(settings.hotkey) && !nativeDisabled;
  $("native-status").textContent = nativeBusy
    ? "Restarting Explorer…"
    : nativeConfirm
      ? "Explorer (taskbar and desktop) restarts for a moment. Open File Explorer windows will close."
      : nativeDisabled
        ? "Disabled. Win + V opens only Simple Clipboard."
        : conflict
          ? "Still active: Windows opens its own clipboard history on Win + V too. Disable it so only Simple Clipboard opens."
          : "Active (Windows default).";
  btn.textContent = nativeConfirm ? "Restart Explorer now" : nativeDisabled ? "Restore Windows default" : "Disable Windows Win + V";
  btn.className = "btn " + (nativeConfirm || (conflict && !nativeDisabled) ? "filled" : "tonal");
  btn.disabled = nativeBusy;
  $("native-cancel").hidden = !nativeConfirm || nativeBusy;
}

$("native-btn").addEventListener("click", async () => {
  if (!nativeConfirm) {
    nativeConfirm = true;
    return renderNative();
  }
  nativeBusy = true;
  renderNative();
  try {
    nativeDisabled = await invoke("set_native_win_v_disabled", { disabled: !nativeDisabled });
    snackbar(nativeDisabled ? "Windows Win + V disabled" : "Windows Win + V restored");
  } catch (e) {
    snackbar(String(e));
  } finally {
    nativeBusy = false;
    nativeConfirm = false;
    renderNative();
  }
});
$("native-cancel").addEventListener("click", () => {
  nativeConfirm = false;
  renderNative();
});

const KEY_NAMES = { Space: "Space", Backquote: "`", Minus: "-", Equal: "=", Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]", Backslash: "\\", Insert: "Insert", Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown" };

function keyFromCode(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  if (/^Numpad\d$/.test(code)) return code;
  return KEY_NAMES[code] ?? null;
}

document.addEventListener(
  "keydown",
  (ev) => {
    if (!recording) return;
    ev.preventDefault();
    ev.stopPropagation();
    if (ev.key === "Escape") {
      recording = false;
      return renderHotkey();
    }
    if (ev.key === "Backspace") {
      recording = false;
      update("hotkey", "");
      return renderHotkey();
    }
    const key = keyFromCode(ev.code);
    if (!key) return; // modifier only so far
    const mods = [];
    if (ev.ctrlKey) mods.push("Ctrl");
    if (ev.altKey) mods.push("Alt");
    if (ev.shiftKey) mods.push("Shift");
    if (ev.metaKey) mods.push("Super");
    if (!mods.length && !/^F\d+$/.test(key)) {
      snackbar("Use at least one modifier (Ctrl, Alt, Shift or Win)");
      return;
    }
    recording = false;
    update("hotkey", [...mods, key].join("+"));
    renderHotkey();
  },
  true,
);

// Tags
$("tag-list").addEventListener("input", (ev) => {
  const li = ev.target.closest("li[data-index]");
  if (!li) return;
  const tag = settings.tags[Number(li.dataset.index)];
  const field = ev.target.dataset.field;
  tag[field] = ev.target.value;
  if (field === "color") li.querySelector(".color").style.background = ev.target.value;
  scheduleSave(500);
});
$("tag-list").addEventListener("click", (ev) => {
  const btn = ev.target.closest("[data-remove]");
  if (!btn) return;
  settings.tags.splice(Number(btn.closest("li").dataset.index), 1);
  renderTags();
  scheduleSave();
});
$("add-tag").addEventListener("click", () => {
  const palette = ["#1E88E5", "#43A047", "#F4511E", "#8E24AA", "#FDD835", "#00ACC1", "#6D4C41", "#E53935"];
  settings.tags.push({
    id: "t" + Date.now().toString(36),
    name: `Tag ${settings.tags.length + 1}`,
    color: palette[settings.tags.length % palette.length],
  });
  renderTags();
  scheduleSave();
  const inputs = $("tag-list").querySelectorAll("input[data-field=name]");
  inputs[inputs.length - 1]?.select();
});

// Actions
$("reset-position").addEventListener("click", async () => {
  await invoke("reset_popup_position");
  settings.popup_position = null;
  render();
  snackbar("The popup opens at the mouse again");
});

$("clear-history").addEventListener("click", async () => {
  await invoke("clear_history");
  snackbar("History cleared");
});

$("export").addEventListener("click", async () => {
  try {
    const path = await invoke("export_settings");
    if (path) snackbar(`Exported to ${path}`);
  } catch (e) {
    snackbar(String(e));
  }
});
$("import").addEventListener("click", async () => {
  try {
    const imported = await invoke("import_settings");
    if (imported) {
      settings = imported;
      render();
      snackbar("Settings imported");
    }
  } catch (e) {
    snackbar(String(e));
    settings = await invoke("get_settings");
    render();
  }
});

const INSTALL_HINTS = {
  installer: "The installer opens and Simple Clipboard closes. Your history and settings are kept.",
  msi: "The Windows installer opens and Simple Clipboard closes. Your history and settings are kept.",
  portable: "The program file is replaced and Simple Clipboard restarts. Your history and settings are kept.",
  deb: "You'll be asked for your password to install the package. Simple Clipboard restarts afterwards.",
};
let installing = false;

function showUpdate(r) {
  latestRelease = r.available ? r : null;
  $("update-title").textContent = r.available ? `Version ${r.latest} is available` : "You're up to date";
  const canInstall = r.available && r.asset && INSTALL_HINTS[r.install_kind];
  $("update-status").textContent = !r.available
    ? `Version ${r.current} is the latest release`
    : canInstall
      ? `You have ${r.current}. ${INSTALL_HINTS[r.install_kind]}`
      : `You have ${r.current}. Download the new version from the release page.`;
  $("install-update").hidden = !canInstall;
  $("open-release").hidden = !r.available;
  $("open-release").textContent = canInstall ? "Release notes" : "Open release page";
  const notes = (r.notes || "").trim();
  $("release-notes").hidden = !r.available || !notes;
  $("release-notes").textContent = notes;
}

async function checkUpdates() {
  if (installing) return;
  const btn = $("check-updates");
  btn.disabled = true;
  $("update-status").textContent = "Checking…";
  try {
    showUpdate(await invoke("check_updates"));
  } catch (e) {
    latestRelease = null;
    $("update-title").textContent = "Couldn't check for updates";
    $("update-status").textContent = String(e);
    $("install-update").hidden = $("open-release").hidden = $("release-notes").hidden = true;
  } finally {
    btn.disabled = false;
  }
}

function formatMb(bytes) {
  return (bytes / 1048576).toFixed(1) + " MB";
}

listen("update-progress", (ev) => {
  const p = ev.payload;
  const box = $("update-progress");
  box.hidden = false;
  if (p.installing) {
    box.classList.add("indeterminate");
    $("update-fill").style.width = "";
    $("update-progress-text").textContent = "Starting installer…";
    return;
  }
  box.classList.toggle("indeterminate", !p.total);
  $("update-fill").style.width = p.total ? `${Math.min(100, (p.downloaded / p.total) * 100)}%` : "";
  $("update-progress-text").textContent = p.total
    ? `${formatMb(p.downloaded)} of ${formatMb(p.total)}`
    : formatMb(p.downloaded);
});

$("install-update").addEventListener("click", async () => {
  installing = true;
  for (const id of ["install-update", "check-updates"]) $(id).disabled = true;
  $("update-title").textContent = `Updating to ${latestRelease.latest}…`;
  $("update-status").textContent = "Downloading…";
  $("update-progress").hidden = false;
  $("update-progress").classList.add("indeterminate");
  $("update-progress-text").textContent = "";
  try {
    await invoke("install_update");
    // The app exits on success; this is only reached if it didn't.
  } catch (e) {
    $("update-title").textContent = "Update failed";
    $("update-status").textContent = String(e);
    $("update-progress").hidden = true;
  } finally {
    installing = false;
    for (const id of ["install-update", "check-updates"]) $(id).disabled = false;
  }
});

$("check-updates").addEventListener("click", checkUpdates);
$("open-release").addEventListener("click", () => latestRelease && invoke("open_url", { url: latestRelease.url }));
$("repo-link").addEventListener("click", (ev) => {
  ev.preventDefault();
  invoke("open_url", { url: info.repo });
});

function goToSection(id) {
  const el = document.getElementById(id);
  if (!el) return;
  el.scrollIntoView({ behavior: "smooth", block: "start" });
  el.classList.remove("flash");
  void el.offsetWidth;
  el.classList.add("flash");
  if (id === "updates") checkUpdates();
}

listen("settings-section", (ev) => goToSection(ev.payload));
listen("settings-changed", (ev) => {
  if (saveTimer) return; // our own pending change wins
  settings = ev.payload;
  render();
});
darkQuery.addEventListener("change", () => settings && applyTheme(settings));

(async function init() {
  hydrateIcons();
  [settings, info, nativeDisabled] = await Promise.all([
    invoke("get_settings"),
    invoke("app_info"),
    invoke("native_win_v_disabled"),
  ]);
  $("version").textContent = `v${info.version}`;
  $("update-title").textContent = `Simple Clipboard ${info.version}`;
  const pending = await invoke("pending_update");
  if (pending) showUpdate(pending);
  render();
  const hash = location.hash.slice(1);
  if (hash) setTimeout(() => goToSection(hash), 100);
})();
