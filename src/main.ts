import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface ProviderStatus {
  provider: string;
  has_key: boolean;
  config_enabled: boolean;
  active: boolean;
  used: number | null;
  limit: number | null;
  unit: string;
  error: string | null;
  secondary_used: number | null;
  secondary_limit: number | null;
  secondary_label: string | null;
  primary_label: string | null;
}

interface ProviderConfig {
  provider: string;
  key: string;
  enable: boolean;
}

const KNOWN_PROVIDERS: { id: string; label: string; needsKey: boolean }[] = [
  { id: "openrouter", label: "OpenRouter", needsKey: true },
  { id: "elevenlabs", label: "ElevenLabs", needsKey: true },
  { id: "claude-code", label: "Claude Code", needsKey: false },
];

const REFRESH_INTERVAL_MS = 5 * 60 * 1000;

const mainView = document.getElementById("main-view")!;
const settingsView = document.getElementById("settings-view")!;
const settingsListEl = document.getElementById("settings-list")!;
const saveSettingsBtn = document.getElementById("save-settings-btn")!;
const openConfigFileBtn = document.getElementById("open-config-file-btn")!;
const activeListEl = document.getElementById("active-list")!;
const disabledListEl = document.getElementById("disabled-list")!;
const disabledCountEl = document.getElementById("disabled-count")!;
const disabledCaretEl = document.getElementById("disabled-caret")!;
const disabledToggleBtn = document.getElementById("disabled-toggle")!;
const statusLineEl = document.getElementById("status-line")!;
const lastUpdatedEl = document.getElementById("last-updated")!;
const refreshBtn = document.getElementById("refresh-btn")! as HTMLButtonElement;
const settingsBtn = document.getElementById("settings-btn")!;
const closeBtn = document.getElementById("close-btn")!;

function formatNumber(value: number, unit: string): string {
  if (unit === "USD") {
    return `$${value.toFixed(2)}`;
  }
  if (unit === "%") {
    return `${Math.round(value)}%`;
  }
  if (unit === "tokens") {
    return new Intl.NumberFormat("en-US", {
      notation: "compact",
      maximumFractionDigits: 1,
    }).format(value);
  }
  return new Intl.NumberFormat("en-US").format(Math.round(value));
}

function formatUsedLimit(status: ProviderStatus): string {
  const used = status.used as number;
  if (status.limit === null) {
    return formatNumber(used, status.unit);
  }
  if (status.unit === "tokens") {
    return `${formatNumber(used, status.unit)} today · ${formatNumber(status.limit, status.unit)} week`;
  }
  return `${formatNumber(used, status.unit)} / ${formatNumber(status.limit, status.unit)}`;
}

function formatStatusLine(status: ProviderStatus): string {
  if (status.error) {
    return `${status.provider}: error (${status.error})`;
  }
  if (status.used === null) {
    return `${status.provider}: no data`;
  }
  if (status.provider === "claude-code") {
    const week = Math.round(status.used);
    if (status.secondary_used !== null) {
      return `claude-code: 5h ${Math.round(status.secondary_used)}% · week ${week}%`;
    }
    return `claude-code: week ${week}%`;
  }
  return `${status.provider}: ${formatUsedLimit(status)}`;
}

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function renderBarRow(label: string, used: number, limit: number): HTMLElement {
  const row = document.createElement("div");
  row.className = "bar-row";

  const header = document.createElement("div");
  header.className = "bar-row-label";

  const labelSpan = document.createElement("span");
  labelSpan.textContent = label;
  header.appendChild(labelSpan);

  const pct = limit > 0 ? Math.round((used / limit) * 100) : 0;
  const pctSpan = document.createElement("span");
  pctSpan.className = "bar-pct";
  pctSpan.textContent = `${pct}%`;
  header.appendChild(pctSpan);

  row.appendChild(header);

  const track = document.createElement("div");
  track.className = "bar-track";
  const fill = document.createElement("div");
  fill.className = "bar-fill";
  const clampedPct = Math.min(100, pct);
  if (clampedPct >= 90) fill.classList.add("crit");
  else if (clampedPct >= 70) fill.classList.add("warn");
  fill.style.width = `${clampedPct}%`;
  track.appendChild(fill);
  row.appendChild(track);

  return row;
}

function renderActiveCard(status: ProviderStatus): HTMLElement {
  const card = document.createElement("div");
  card.className = "card";

  const top = document.createElement("div");
  top.className = "card-top";

  const name = document.createElement("span");
  name.className = "card-name";
  name.textContent = status.provider;
  top.appendChild(name);

  if (status.provider !== "claude-code" && status.used !== null && !status.error) {
    const numbers = document.createElement("span");
    numbers.className = "card-numbers";
    numbers.textContent = formatUsedLimit(status);
    top.appendChild(numbers);
  }

  card.appendChild(top);

  if (status.error) {
    const err = document.createElement("div");
    err.className = "card-error";
    err.textContent = status.error;
    card.appendChild(err);
  } else if (status.provider === "claude-code" && status.used !== null && status.limit !== null) {
    card.appendChild(renderBarRow(status.primary_label ?? "Week", status.used, status.limit));
    if (status.secondary_used !== null && status.secondary_limit !== null) {
      card.appendChild(
        renderBarRow(
          status.secondary_label ?? "Session",
          status.secondary_used,
          status.secondary_limit,
        ),
      );
    }
  } else if (status.used !== null && status.limit !== null && status.limit > 0) {
    const pct = Math.min(100, (status.used / status.limit) * 100);
    const track = document.createElement("div");
    track.className = "bar-track";
    const fill = document.createElement("div");
    fill.className = "bar-fill";
    if (pct >= 90) fill.classList.add("crit");
    else if (pct >= 70) fill.classList.add("warn");
    fill.style.width = `${pct}%`;
    track.appendChild(fill);
    card.appendChild(track);
  }

  return card;
}

function renderDisabledCard(status: ProviderStatus): HTMLElement {
  const card = document.createElement("div");
  card.className = "card disabled-card";

  const name = document.createElement("span");
  name.className = "card-name";
  name.textContent = status.provider;
  card.appendChild(name);

  if (!status.has_key) {
    const reason = document.createElement("span");
    reason.className = "disabled-reason";
    reason.textContent = "no key";
    card.appendChild(reason);
  }

  return card;
}

function render(statuses: ProviderStatus[]) {
  activeListEl.innerHTML = "";
  disabledListEl.innerHTML = "";

  const active = statuses.filter((s) => s.active);
  const disabled = statuses.filter((s) => !s.active);

  statusLineEl.textContent = active.length === 0 ? "No provider enabled yet." : "";
  statusLineEl.hidden = active.length !== 0;

  for (const status of active) {
    activeListEl.appendChild(renderActiveCard(status));
  }
  for (const status of disabled) {
    disabledListEl.appendChild(renderDisabledCard(status));
  }

  disabledCountEl.textContent = String(disabled.length);
  lastUpdatedEl.textContent = `Updated at ${new Date().toLocaleTimeString("en-US")}`;
}

async function refresh() {
  refreshBtn.classList.add("spinning");
  refreshBtn.disabled = true;

  try {
    const statuses = await invoke<ProviderStatus[]>("fetch_all_usage");
    render(statuses);

    const active = statuses.filter((s) => s.active);
    const lines = active.map(formatStatusLine);
    const tooltip = truncate(
      lines.length > 0 ? lines.join("\n") : "No provider enabled",
      120,
    );

    try {
      await invoke("update_tray_status", { tooltip, lines });
    } catch (e) {
      console.error("failed to update tray", e);
    }
  } catch (e) {
    statusLineEl.hidden = false;
    statusLineEl.textContent = `Failed to load: ${e}`;
  } finally {
    refreshBtn.classList.remove("spinning");
    refreshBtn.disabled = false;
  }
}

function openSettingsView(configs: ProviderConfig[]) {
  settingsListEl.innerHTML = "";

  for (const known of KNOWN_PROVIDERS) {
    const existing = configs.find((c) => c.provider === known.id);

    const row = document.createElement("div");
    row.className = "settings-row";
    row.dataset.provider = known.id;

    const topRow = document.createElement("div");
    topRow.className = "settings-row-top";

    const label = document.createElement("span");
    label.className = "settings-label";
    label.textContent = known.label;
    topRow.appendChild(label);

    const switchLabel = document.createElement("label");
    switchLabel.className = "switch";
    const enableCheckbox = document.createElement("input");
    enableCheckbox.type = "checkbox";
    enableCheckbox.checked = existing?.enable ?? false;
    enableCheckbox.addEventListener("change", async () => {
      enableCheckbox.disabled = true;
      await saveSettings({ keepOpen: true });
      enableCheckbox.disabled = false;
    });
    const slider = document.createElement("span");
    slider.className = "switch-slider";
    switchLabel.appendChild(enableCheckbox);
    switchLabel.appendChild(slider);
    topRow.appendChild(switchLabel);

    row.appendChild(topRow);

    if (known.needsKey) {
      const keyInput = document.createElement("input");
      keyInput.type = "password";
      keyInput.className = "settings-key-input";
      keyInput.placeholder = "API key";
      keyInput.value = existing?.key ?? "";
      row.appendChild(keyInput);
    }

    settingsListEl.appendChild(row);
  }

  mainView.hidden = true;
  settingsView.hidden = false;
  settingsBtn.textContent = "←";
  settingsBtn.title = "Back";
}

function closeSettingsView() {
  settingsView.hidden = true;
  mainView.hidden = false;
  settingsBtn.textContent = "⚙";
  settingsBtn.title = "Settings";
}

function collectConfigsFromForm(): ProviderConfig[] {
  const rows = settingsListEl.querySelectorAll<HTMLElement>(".settings-row");
  const configs: ProviderConfig[] = [];

  rows.forEach((row) => {
    const provider = row.dataset.provider!;
    const keyInput = row.querySelector<HTMLInputElement>(".settings-key-input");
    const enableCheckbox = row.querySelector<HTMLInputElement>(
      "input[type=checkbox]",
    )!;
    configs.push({
      provider,
      key: keyInput?.value.trim() ?? "",
      enable: enableCheckbox.checked,
    });
  });

  return configs;
}

async function saveSettings(options?: { keepOpen?: boolean }) {
  const configs = collectConfigsFromForm();

  if (configs.length !== KNOWN_PROVIDERS.length) {
    alert("Internal error: the settings form did not load correctly. Nothing was saved.");
    return;
  }

  try {
    await invoke("save_config", { configs });
    if (!options?.keepOpen) {
      closeSettingsView();
    }
    await refresh();
  } catch (e) {
    alert(`Failed to save: ${e}`);
  }
}

async function init() {
  await refresh();
  setInterval(refresh, REFRESH_INTERVAL_MS);
  await listen("refresh-requested", () => refresh());
}

refreshBtn.addEventListener("click", () => refresh());

closeBtn.addEventListener("click", () => {
  getCurrentWindow().hide();
});

settingsBtn.addEventListener("click", async () => {
  if (!settingsView.hidden) {
    closeSettingsView();
    return;
  }
  try {
    const configs = await invoke<ProviderConfig[]>("get_config");
    openSettingsView(configs);
  } catch (e) {
    alert(`Failed to load config: ${e}`);
  }
});

saveSettingsBtn.addEventListener("click", () => saveSettings());

openConfigFileBtn.addEventListener("click", async () => {
  try {
    await invoke("open_config_file");
  } catch (e) {
    alert(`Failed to open config.json: ${e}`);
  }
});

let disabledExpanded = false;
disabledToggleBtn.addEventListener("click", () => {
  disabledExpanded = !disabledExpanded;
  disabledListEl.hidden = !disabledExpanded;
  disabledCaretEl.textContent = disabledExpanded ? "▾" : "▸";
});

init();
