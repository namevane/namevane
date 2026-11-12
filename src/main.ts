import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { language, setLanguage, t } from "./i18n";

type Pin = {
  id: string;
  name: string;
  addresses: string[];
  enabled: boolean;
};

type PinList = {
  pins: Pin[];
  error: string | null;
};

type PinInput = {
  id: string | null;
  name: string;
  target: string;
  otherAddress: string;
  enabled: boolean;
};

const form = document.querySelector<HTMLFormElement>("#pin-form");
const nameInput = document.querySelector<HTMLInputElement>("#name");
const ipv4Input = document.querySelector<HTMLInputElement>("#ipv4");
const ipv6Input = document.querySelector<HTMLInputElement>("#ipv6");
const enabledInput = document.querySelector<HTMLInputElement>("#enabled");
const saveButton = document.querySelector<HTMLButtonElement>("#save");
const cancelButton = document.querySelector<HTMLButtonElement>("#cancel");
const loadError = document.querySelector<HTMLElement>("#load-error");
const notice = document.querySelector<HTMLElement>("#notice");
const pageTitle = document.querySelector<HTMLElement>("#page-title");
const updateNote = document.querySelector<HTMLElement>("#update-note");
const updateNoteText = document.querySelector<HTMLElement>("#update-note-text");
const updateInstall = document.querySelector<HTMLButtonElement>("#update-install");
const empty = document.querySelector<HTMLElement>("#empty");
const hostsState = document.querySelector<HTMLElement>("#hosts-state");
const applyButton = document.querySelector<HTMLButtonElement>("#apply");
const turnOffButton = document.querySelector<HTMLButtonElement>("#turn-off");
const list = document.querySelector<HTMLUListElement>("#pins");
const aboutOpen = document.querySelector<HTMLButtonElement>("#about-open");
const aboutDialog = document.querySelector<HTMLDialogElement>("#about");
const aboutVersion = document.querySelector<HTMLElement>("#about-version");
const aboutSource = document.querySelector<HTMLButtonElement>("#about-source");
const aboutReleases = document.querySelector<HTMLButtonElement>("#about-releases");
const backgroundInput = document.querySelector<HTMLInputElement>("#background");
const loginInput = document.querySelector<HTMLInputElement>("#login");
const exportButton = document.querySelector<HTMLButtonElement>("#export-pins");
const importButton = document.querySelector<HTMLButtonElement>("#import-pins");
const settingsVersion = document.querySelector<HTMLElement>("#settings-version");
const settingsUpdate = document.querySelector<HTMLElement>("#settings-update");
const settingsInstall = document.querySelector<HTMLButtonElement>("#settings-install");
const settingsReleases = document.querySelector<HTMLButtonElement>("#settings-releases");
const languageSelect = document.querySelector<HTMLSelectElement>("#language");
const listForm = document.querySelector<HTMLFormElement>("#list-form");
const listUrlInput = document.querySelector<HTMLInputElement>("#list-url");
const listTitleInput = document.querySelector<HTMLInputElement>("#list-title");
const listInterval = document.querySelector<HTMLSelectElement>("#list-interval");
const listCheck = document.querySelector<HTMLButtonElement>("#list-check");
const listAdd = document.querySelector<HTMLButtonElement>("#list-add");
const listsEmpty = document.querySelector<HTMLElement>("#lists-empty");
const sourcesList = document.querySelector<HTMLUListElement>("#sources");

type RuntimeSettings = {
  runInBackground: boolean;
  openAtLogin: boolean;
  language: string;
};

type AboutUpdate =
  | { kind: "none" }
  | { kind: "current" }
  | { kind: "available"; version: string; url: string }
  | { kind: "unknown"; message: string };

type HostsStatus = {
  applied: boolean;
  blockPresent: boolean;
};

type PageName = "addresses" | "lists" | "settings";

type SourceEntry = {
  name: string;
  addresses: string[];
  skipped: boolean;
};

type Source = {
  id: string;
  title: string;
  url: string;
  enabled: boolean;
  version: string;
  names: number;
  checkedAt: number;
  error: string | null;
  entries: SourceEntry[];
};

type SourceList = {
  lists: Source[];
  checkMinutes: number;
  error: string | null;
};

type CheckReport = {
  changed: number;
  unchanged: number;
  failed: number;
  sources: SourceList;
};

type About = {
  version: string;
  repository: string;
  releases: string;
  update: AboutUpdate;
};

let editingId: string | null = null;
let pins: Pin[] = [];
let lastList: PinList = { pins: [], error: null };
let sources: Source[] = [];
let lastSources: SourceList = { lists: [], checkMinutes: 0, error: null };
let page: PageName = "addresses";
let saving = false;
let checkingLists = false;
let noticeTimer = 0;
let installing = false;

function message(error: unknown): string {
  const raw = typeof error === "string"
    ? error
    : error && typeof error === "object" && "message" in error && typeof error.message === "string"
      ? error.message
      : "";
  if (raw.length === 0) {
    return t("error.listSave");
  }
  const split = raw.indexOf("|");
  if (split > 0) {
    return t(raw.slice(0, split), { line: raw.slice(split + 1) });
  }
  return t(raw);
}

function showError(node: HTMLElement | null, text: string): void {
  if (!node) {
    return;
  }
  node.textContent = text;
  node.hidden = text.length === 0;
}

function notify(text: string, isError = false): void {
  if (!notice) {
    return;
  }
  notice.textContent = text;
  notice.hidden = text.length === 0;
  notice.classList.toggle("is-error", isError);
  window.clearTimeout(noticeTimer);
  if (!isError && text.length > 0) {
    noticeTimer = window.setTimeout(() => {
      notice.hidden = true;
    }, 4000);
  }
}

function describe(pin: Pin): string {
  if (pin.addresses.length === 2) {
    return t("list.both", { first: pin.addresses[0] ?? "", second: pin.addresses[1] ?? "" });
  }
  if (pin.addresses.length === 1) {
    return t("list.one", { address: pin.addresses[0] ?? "" });
  }
  return t("list.none");
}

function applyTexts(): void {
  document.documentElement.lang = language();
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((node) => {
    const key = node.dataset.i18n;
    if (key && node.id !== "page-title" && node.id !== "save") {
      node.textContent = t(key);
    }
  });
  document.querySelectorAll<HTMLInputElement>("[data-i18n-placeholder]").forEach((node) => {
    const key = node.dataset.i18nPlaceholder;
    if (key) {
      node.placeholder = t(key);
    }
  });
  if (pageTitle) {
    const titleKey = page === "settings" ? "page.settings" : page === "lists" ? "page.lists" : "page.addresses";
    pageTitle.textContent = t(titleKey);
  }
  if (saveButton) {
    saveButton.textContent = t(editingId ? "action.save" : "action.add");
  }
  if (languageSelect) {
    languageSelect.value = setLanguage(languageSelect.value);
  }
}

function resetForm(): void {
  editingId = null;
  form?.reset();
  if (enabledInput) {
    enabledInput.checked = true;
  }
  if (saveButton) {
    saveButton.textContent = t("action.add");
  }
  if (cancelButton) {
    cancelButton.hidden = true;
  }
}

function fillForm(pin: Pin): void {
  if (!nameInput || !ipv4Input || !ipv6Input || !enabledInput || !saveButton || !cancelButton) {
    return;
  }
  editingId = pin.id;
  nameInput.value = pin.name;
  ipv4Input.value = "";
  ipv6Input.value = "";
  for (const address of pin.addresses) {
    if (address.includes(":")) {
      ipv6Input.value = address;
    } else {
      ipv4Input.value = address;
    }
  }
  enabledInput.checked = pin.enabled;
  saveButton.textContent = t("action.save");
  cancelButton.hidden = false;
  nameInput.focus();
}

function pinInput(): PinInput | null {
  if (!nameInput || !ipv4Input || !ipv6Input || !enabledInput) {
    return null;
  }
  const addresses = [ipv4Input.value.trim(), ipv6Input.value.trim()].filter((value) => value.length > 0);
  if (addresses.length === 0) {
    notify(t("notice.needAddress"), true);
    ipv4Input.focus();
    return null;
  }
  return {
    id: editingId,
    name: nameInput.value,
    target: addresses[0] ?? "",
    otherAddress: addresses[1] ?? "",
    enabled: enabledInput.checked,
  };
}

function render(loaded: PinList): void {
  lastList = loaded;
  pins = loaded.pins;
  const blocked = loaded.error !== null;
  if (form) {
    form.hidden = blocked;
  }
  showError(loadError, loaded.error ?? "");
  if (!list || !empty) {
    return;
  }
  list.replaceChildren();
  empty.hidden = blocked || pins.length > 0;
  list.hidden = pins.length === 0;
  for (const pin of pins) {
    list.append(renderPin(pin));
  }
}

function renderPin(pin: Pin): HTMLLIElement {
  const item = document.createElement("li");
  item.className = pin.enabled ? "pin" : "pin is-off";
  item.dataset.id = pin.id;

  const toggle = document.createElement("label");
  toggle.className = "switch";
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.checked = pin.enabled;
  checkbox.dataset.action = "toggle";
  const state = document.createElement("span");
  state.textContent = pin.enabled ? t("state.on") : t("state.off");
  toggle.append(checkbox, state);

  const text = document.createElement("div");
  text.className = "pin-text";
  const name = document.createElement("strong");
  name.textContent = pin.name;
  const target = document.createElement("span");
  target.textContent = describe(pin);
  text.append(name, target);

  const actions = document.createElement("div");
  actions.className = "pin-actions";
  const edit = document.createElement("button");
  edit.type = "button";
  edit.dataset.action = "edit";
  edit.textContent = t("action.edit");
  const remove = document.createElement("button");
  remove.type = "button";
  remove.dataset.action = "remove";
  remove.textContent = t("action.remove");
  actions.append(edit, remove);

  item.append(toggle, text, actions);
  return item;
}

async function refresh(): Promise<void> {
  const loaded = await invoke<PinList>("list_pins");
  render(loaded);
  await loadSources();
  await loadHosts();
}

function showHosts(status: HostsStatus): void {
  if (applyButton) {
    applyButton.hidden = status.applied;
  }
  if (turnOffButton) {
    turnOffButton.hidden = !status.blockPresent;
  }
  if (!hostsState) {
    return;
  }
  const parts: string[] = [];
  if (!status.applied) {
    parts.push(t("hosts.notApplied"));
  }
  hostsState.hidden = parts.length === 0;
  hostsState.textContent = parts.join(" ");
}

async function loadHosts(): Promise<void> {
  showHosts(await invoke<HostsStatus>("hosts_status"));
}

function rawMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return "";
}

async function changeHosts(command: "apply_pins" | "turn_off_pins"): Promise<void> {
  try {
    const status = await invoke<HostsStatus>(command);
    showHosts(status);
    notify(t(command === "apply_pins" ? "hosts.applied" : "hosts.turnedOff"));
  } catch (error: unknown) {
    const raw = rawMessage(error);
    notify(message(error), raw !== "hosts.cancelled");
    await loadHosts().catch(() => undefined);
  }
}

async function savePin(input: PinInput): Promise<void> {
  await invoke("upsert_pin", { pin: input });
}

function updateSentence(update: AboutUpdate): string {
  switch (update.kind) {
    case "none":
      return t("update.none");
    case "current":
      return t("update.current");
    case "available":
      return t("update.available", { version: update.version });
    case "unknown":
      return t(update.message);
  }
}

function openPage(url: string): void {
  void invoke("open_project_page", { url }).catch((error: unknown) => {
    notify(message(error), true);
  });
}

function applyRuntime(settings: RuntimeSettings): void {
  if (backgroundInput) {
    backgroundInput.checked = settings.runInBackground;
  }
  if (loginInput) {
    loginInput.checked = settings.openAtLogin;
  }
  if (languageSelect) {
    languageSelect.value = settings.language === "bg" ? "bg" : "en";
  }
  setLanguage(settings.language);
  applyTexts();
  render(lastList);
  renderSources(lastSources);
  void loadHosts().catch(() => undefined);
}

async function loadRuntime(): Promise<void> {
  applyRuntime(await invoke<RuntimeSettings>("runtime_settings"));
}

async function changeRuntime(
  command: "set_run_in_background" | "set_open_at_login",
  enabled: boolean,
): Promise<void> {
  try {
    applyRuntime(await invoke<RuntimeSettings>(command, { enabled }));
    notify(t("notice.setting"));
  } catch (error: unknown) {
    notify(message(error), true);
    await loadRuntime().catch(() => undefined);
  }
}

function showUpdate(about: About): void {
  const available = about.update.kind === "available";
  if (aboutVersion) {
    aboutVersion.textContent = about.version;
  }
  if (settingsVersion) {
    settingsVersion.textContent = about.version;
  }
  if (settingsUpdate) {
    settingsUpdate.textContent = updateSentence(about.update);
  }
  if (updateNote && updateNoteText) {
    updateNote.hidden = !available;
    if (about.update.kind === "available") {
      updateNoteText.textContent = t("update.available", { version: about.update.version });
    }
  }
  if (settingsInstall) {
    settingsInstall.hidden = !available;
  }
  aboutSource?.setAttribute("data-url", about.repository);
  aboutReleases?.setAttribute("data-url", about.releases);
  settingsReleases?.setAttribute("data-url", about.releases);
}

async function loadAbout(): Promise<void> {
  const about = await invoke<About>("app_about");
  showUpdate(about);
}

async function installUpdate(): Promise<void> {
  if (installing) {
    return;
  }
  installing = true;
  notify(t("notice.downloading"));
  try {
    await invoke("install_update");
    notify(t("notice.reopen"));
  } catch (error: unknown) {
    notify(message(error), true);
    installing = false;
  }
}

function showPage(name: PageName): void {
  page = name;
  document.querySelector("#page-addresses")?.toggleAttribute("hidden", name !== "addresses");
  document.querySelector("#page-lists")?.toggleAttribute("hidden", name !== "lists");
  document.querySelector("#page-settings")?.toggleAttribute("hidden", name !== "settings");
  if (pageTitle) {
    const titleKey = name === "settings" ? "page.settings" : name === "lists" ? "page.lists" : "page.addresses";
    pageTitle.textContent = t(titleKey);
  }
  for (const tab of document.querySelectorAll<HTMLButtonElement>(".tab")) {
    const selected = tab.dataset.page === name;
    tab.classList.toggle("is-selected", selected);
    tab.setAttribute("aria-selected", String(selected));
  }
}

function entryLine(entry: SourceEntry): string {
  return `${entry.name} — ${entry.addresses.join(", ")}`;
}

function renderSources(loaded: SourceList): void {
  lastSources = loaded;
  sources = loaded.lists;
  if (listsEmpty) {
    listsEmpty.hidden = loaded.lists.length > 0 || loaded.error !== null;
  }
  if (listInterval && document.activeElement !== listInterval) {
    listInterval.value = String(loaded.checkMinutes);
  }
  if (!sourcesList) {
    return;
  }
  sourcesList.replaceChildren();
  if (loaded.error) {
    const item = document.createElement("li");
    item.className = "error";
    item.textContent = message(loaded.error);
    sourcesList.append(item);
  }
  for (const source of loaded.lists) {
    sourcesList.append(sourceItem(source));
  }
}

function sourceItem(source: Source): HTMLLIElement {
  const item = document.createElement("li");
  item.className = source.enabled ? "pin" : "pin is-off";
  item.dataset.id = source.id;

  const toggle = document.createElement("label");
  toggle.className = "switch";
  const box = document.createElement("input");
  box.type = "checkbox";
  box.checked = source.enabled;
  box.dataset.action = "toggle-list";
  const state = document.createElement("span");
  state.textContent = t(source.enabled ? "state.on" : "state.off");
  toggle.append(box, state);

  const text = document.createElement("div");
  text.className = "pin-text";
  const title = document.createElement("strong");
  title.textContent = source.title;
  const meta = document.createElement("span");
  const checked = source.checkedAt > 0
    ? t("lists.checked", { time: new Date(source.checkedAt * 1000).toLocaleString() })
    : "";
  meta.textContent = checked.length > 0
    ? `${t("lists.meta", { count: source.names, version: source.version })} · ${checked}`
    : t("lists.meta", { count: source.names, version: source.version });
  const url = document.createElement("span");
  url.textContent = source.url;
  text.append(title, meta, url);

  const skipped = source.entries.filter((entry) => entry.skipped).length;
  if (!source.enabled) {
    const off = document.createElement("span");
    off.textContent = t("lists.off");
    text.append(off);
  } else if (skipped > 0) {
    const note = document.createElement("span");
    note.textContent = t("lists.skipped", { count: skipped });
    text.append(note);
  }
  if (source.error) {
    const problem = document.createElement("span");
    problem.className = "error";
    problem.textContent = message(source.error);
    text.append(problem);
  }
  const visible = source.entries.filter((entry) => !entry.skipped).slice(0, 6);
  if (visible.length > 0) {
    const names = document.createElement("ul");
    names.className = "source-lines";
    for (const entry of visible) {
      const row = document.createElement("li");
      row.textContent = entryLine(entry);
      names.append(row);
    }
    const hidden = source.entries.length - skipped - visible.length;
    if (hidden > 0) {
      const more = document.createElement("li");
      more.textContent = t("lists.more", { count: hidden });
      names.append(more);
    }
    text.append(names);
  }

  const actions = document.createElement("div");
  actions.className = "pin-actions";
  const check = document.createElement("button");
  check.type = "button";
  check.dataset.action = "check-list";
  check.textContent = t("action.checkOne");
  const remove = document.createElement("button");
  remove.type = "button";
  remove.dataset.action = "remove-list";
  remove.textContent = t("action.remove");
  actions.append(check, remove);

  item.append(toggle, text, actions);
  return item;
}

function checkNotice(report: CheckReport): string {
  if (report.changed > 0) {
    return t("notice.listsUpdated");
  }
  if (report.failed > 0) {
    return t("notice.listsFailed");
  }
  return t("notice.listsSame");
}

async function loadSources(): Promise<void> {
  renderSources(await invoke<SourceList>("list_sources"));
}

async function checkLists(id: string | null): Promise<void> {
  if (checkingLists) {
    return;
  }
  checkingLists = true;
  if (listCheck) {
    listCheck.disabled = true;
  }
  if (listAdd) {
    listAdd.disabled = true;
  }
  try {
    const report = await invoke<CheckReport>("check_sources", { id });
    renderSources(report.sources);
    await loadHosts();
    notify(checkNotice(report), report.failed > 0 && report.changed === 0);
  } catch (error: unknown) {
    notify(message(error), true);
  } finally {
    checkingLists = false;
    if (listCheck) {
      listCheck.disabled = false;
    }
    if (listAdd) {
      listAdd.disabled = false;
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  form?.addEventListener("submit", (event) => {
    event.preventDefault();
    if (saving || !nameInput) {
      return;
    }
    const input = pinInput();
    if (!input) {
      return;
    }
    saving = true;
    const wasEditing = editingId !== null;
    const savedName = nameInput.value.trim();
    void savePin(input)
      .then(async () => {
        resetForm();
        await refresh();
        notify(wasEditing ? t("notice.saved", { name: savedName }) : t("notice.added", { name: savedName }));
        nameInput.focus();
      })
      .catch((error: unknown) => {
        notify(message(error), true);
      })
      .finally(() => {
        saving = false;
      });
  });

  applyButton?.addEventListener("click", () => {
    void changeHosts("apply_pins");
  });
  turnOffButton?.addEventListener("click", () => {
    void changeHosts("turn_off_pins");
  });

  cancelButton?.addEventListener("click", () => {
    resetForm();
    nameInput?.focus();
  });

  list?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    const action = target.dataset.action;
    const item = target.closest<HTMLLIElement>("li");
    const id = item?.dataset.id;
    if (!action || !id) {
      return;
    }
    const pin = pins.find((entry) => entry.id === id);
    if (!pin) {
      return;
    }
    if (action === "edit") {
      fillForm(pin);
      return;
    }
    if (action === "remove") {
      void invoke("delete_pin", { id })
        .then(async () => {
          if (editingId === id) {
            resetForm();
          }
          await refresh();
          notify(t("notice.removed", { name: pin.name }));
        })
        .catch((error: unknown) => {
          notify(message(error), true);
        });
    }
  });

  list?.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement) || target.dataset.action !== "toggle") {
      return;
    }
    const item = target.closest<HTMLLIElement>("li");
    const id = item?.dataset.id;
    if (!id) {
      return;
    }
    void invoke("set_pin_enabled", { id, enabled: target.checked })
      .then(() => refresh())
      .catch((error: unknown) => {
        notify(message(error), true);
        return refresh();
      });
  });

  document.querySelector(".tabs")?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLButtonElement) || !target.dataset.page) {
      return;
    }
    const name = target.dataset.page;
    if (name === "settings" || name === "lists" || name === "addresses") {
      showPage(name);
    }
  });

  exportButton?.addEventListener("click", () => {
    void invoke<boolean>("export_pins")
      .then((saved) => {
        if (saved) {
          notify(t("notice.exported"));
        }
      })
      .catch((error: unknown) => notify(message(error), true));
  });

  importButton?.addEventListener("click", () => {
    void invoke<{ added: number; skipped: number }>("import_pins")
      .then((report) => {
        if (report.added === 0) {
          return;
        }
        const count = report.added === 1
          ? t("notice.importedOne")
          : t("notice.importedMany", { count: report.added });
        const skipped = report.skipped > 0 ? ` ${t("notice.skipped", { count: report.skipped })}` : "";
        notify(`${count}${skipped}`);
        return refresh();
      })
      .catch((error: unknown) => notify(message(error), true));
  });

  updateInstall?.addEventListener("click", () => {
    void installUpdate();
  });
  settingsInstall?.addEventListener("click", () => {
    void installUpdate();
  });

  aboutOpen?.addEventListener("click", () => {
    aboutDialog?.showModal();
    void loadAbout().catch((error: unknown) => notify(message(error), true));
  });

  for (const button of [aboutSource, aboutReleases, settingsReleases]) {
    button?.addEventListener("click", () => {
      const url = button.dataset.url;
      if (url) {
        openPage(url);
      }
    });
  }

  backgroundInput?.addEventListener("change", () => {
    if (!backgroundInput) {
      return;
    }
    void changeRuntime("set_run_in_background", backgroundInput.checked);
  });

  loginInput?.addEventListener("change", () => {
    if (!loginInput) {
      return;
    }
    void changeRuntime("set_open_at_login", loginInput.checked);
  });

  listForm?.addEventListener("submit", (event) => {
    event.preventDefault();
    if (checkingLists || !listUrlInput) {
      return;
    }
    const url = listUrlInput.value.trim();
    const title = listTitleInput?.value.trim() ?? "";
    if (url.length === 0) {
      notify(t("error.listUrl"), true);
      return;
    }
    checkingLists = true;
    if (listAdd) {
      listAdd.disabled = true;
    }
    if (listCheck) {
      listCheck.disabled = true;
    }
    void invoke<SourceList>("add_source", { source: { title, url } })
      .then(async (loaded) => {
        renderSources(loaded);
        listForm?.reset();
        if (listInterval) {
          listInterval.value = String(loaded.checkMinutes);
        }
        await loadHosts();
        const added = loaded.lists[loaded.lists.length - 1];
        notify(t("notice.listAdded", { name: added?.title ?? url }));
        listUrlInput.focus();
      })
      .catch((error: unknown) => {
        notify(message(error), true);
      })
      .finally(() => {
        checkingLists = false;
        if (listAdd) {
          listAdd.disabled = false;
        }
        if (listCheck) {
          listCheck.disabled = false;
        }
      });
  });

  listCheck?.addEventListener("click", () => {
    void checkLists(null);
  });

  listInterval?.addEventListener("change", () => {
    if (!listInterval) {
      return;
    }
    const minutes = Number(listInterval.value);
    void invoke<SourceList>("set_list_interval", { minutes })
      .then((loaded) => {
        renderSources(loaded);
        notify(t("notice.setting"));
      })
      .catch((error: unknown) => {
        notify(message(error), true);
        return loadSources();
      });
  });

  sourcesList?.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    const action = target.dataset.action;
    const item = target.closest<HTMLLIElement>("li");
    const id = item?.dataset.id;
    if (!action || !id) {
      return;
    }
    const source = sources.find((entry) => entry.id === id);
    if (!source) {
      return;
    }
    if (action === "check-list") {
      void checkLists(id);
      return;
    }
    if (action === "remove-list") {
      void invoke<SourceList>("remove_source", { id })
        .then(async (loaded) => {
          renderSources(loaded);
          await loadHosts();
          notify(t("notice.listRemoved", { name: source.title }));
        })
        .catch((error: unknown) => notify(message(error), true));
    }
  });

  sourcesList?.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement) || target.dataset.action !== "toggle-list") {
      return;
    }
    const item = target.closest<HTMLLIElement>("li");
    const id = item?.dataset.id;
    if (!id) {
      return;
    }
    void invoke<SourceList>("set_source_enabled", { id, enabled: target.checked })
      .then(async (loaded) => {
        renderSources(loaded);
        await loadHosts();
      })
      .catch((error: unknown) => {
        notify(message(error), true);
        return loadSources();
      });
  });

  void listen<{ changed: number; failed: number; automatic: boolean }>("lists-updated", (event) => {
    void loadSources().then(() => loadHosts()).catch((error: unknown) => notify(message(error), true));
    if (event.payload.automatic && event.payload.changed > 0) {
      notify(t("notice.listsUpdated"));
    } else if (event.payload.automatic && event.payload.failed > 0) {
      notify(t("notice.listsFailed"), true);
    }
  }).catch(() => undefined);

  void refresh().catch((error: unknown) => {
    showError(loadError, message(error));
  });
  void loadRuntime().catch((error: unknown) => notify(message(error), true));
  languageSelect?.addEventListener("change", () => {
    if (!languageSelect) {
      return;
    }
    setLanguage(languageSelect.value);
    applyTexts();
    render(lastList);
    renderSources(lastSources);
    void invoke("set_language", { language: languageSelect.value }).catch((error: unknown) => {
      notify(message(error), true);
    });
  });

  applyTexts();

  void loadAbout().catch((error: unknown) => {
    if (settingsUpdate) {
      settingsUpdate.textContent = message(error);
    }
  });
});
