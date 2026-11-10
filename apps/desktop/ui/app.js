const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);
const t = (key) => window.NexusQI18n.t(key);
const feedback = $("feedback");
const statusLabel = $("vault-status");
const statusDot = $("status-dot");
const lockButton = $("lock-button");
let vaultUnlocked = false;
const operationList = $("operation-list");
const operationCards = new Map();
const trackedCommands = new Set(["encrypt_file", "decrypt_file", "encrypt_folder", "decrypt_folder", "export_backup", "import_backup", "create_shared_package", "open_shared_package"]);
const operationTitles = {
  encrypt_file: "Encrypting file", decrypt_file: "Restoring file",
  encrypt_folder: "Encrypting folder", decrypt_folder: "Restoring folder",
  export_backup: "Creating key backup", import_backup: "Restoring key backup",
  create_shared_package: "Creating shared package", open_shared_package: "Opening shared package",
};
const operationContexts = {
  encrypt_file: "Source file → encrypted .nqx output",
  decrypt_file: "Encrypted .nqx input → restored file",
  encrypt_folder: "Source folder → encrypted folder vault",
  decrypt_folder: "Folder vault → restored folder",
  export_backup: "Vault state and keys → .nqb backup",
  import_backup: ".nqb backup → new vault",
  create_shared_package: "Source file → recipient-encrypted .nqx package",
  open_shared_package: "Encrypted .nqx package → recovered file",
};
const phaseTitles = {
  preparing: "Preparing", readingInput: "Reading input", processing: "Processing",
  writingOutput: "Writing output", verifying: "Verifying", completed: "Completed",
};
function createOperationCard(operationId, command) {
  if (!operationList) return;
  const empty = operationList.querySelector(".operation-empty");
  if (empty) empty.remove();
  const item = document.createElement("article");
  item.className = "operation-item";
  const title = document.createElement("span");
  title.className = "operation-item-title";
  title.textContent = t(operationTitles[command] || "Vault operation");
  const context = document.createElement("span");
  context.className = "operation-item-context";
  context.textContent = t(operationContexts[command] || "Local vault operation");
  const status = document.createElement("span");
  status.className = "operation-item-status";
  status.setAttribute("aria-live", "polite");
  status.dataset.state = "queued";
  status.textContent = t("Queued");
  const phase = document.createElement("span");
  phase.className = "operation-item-phase";
  phase.textContent = "";
  const progress = document.createElement("progress");
  progress.removeAttribute("value");
  progress.max = 100;
  progress.setAttribute("aria-label", t("Operation progress"));
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.dataset.translationKey = "Cancel";
  cancel.textContent = t(cancel.dataset.translationKey);
  cancel.addEventListener("click", async () => {
    cancel.disabled = true;
    cancel.dataset.translationKey = "Requesting…";
    cancel.textContent = t(cancel.dataset.translationKey);
    try {
      const accepted = await invoke("cancel_operation", { operationId });
      if (!accepted) cancel.hidden = true;
    } catch (error) {
      cancel.disabled = false;
      cancel.dataset.translationKey = "Cancel";
      cancel.textContent = t(cancel.dataset.translationKey);
      showMessage(String(error), true);
    }
  });
  item.append(title, status, context, phase, progress, cancel);
  operationList.prepend(item);
  operationCards.set(operationId, { item, title, context, status, phase, progress, cancel, command });
  refreshLockButtonAvailability();
  while (operationList.children.length > 8) {
    const stale = [...operationCards.entries()].find(([, card]) =>
      ["completed", "failed", "cancelled"].includes(card.status.dataset.state));
    if (!stale) break;
    stale[1].item.remove();
    operationCards.delete(stale[0]);
  }
}
function updateOperationCard(payload) {
  const card = operationCards.get(payload.operationId);
  if (!card) return;
  const states = ({ queued: t("Queued"), running: t("Running"), cancelling: t("Cancelling"), completed: t("Completed"), failed: t("Failed"), cancelled: t("Cancelled") });
  const currentStatus = card.status.dataset.state;
  const preserveCancellation = currentStatus === "cancelling" && payload.status === "running";
  if (!preserveCancellation) {
    card.status.textContent = states[payload.status] || payload.status;
    card.status.dataset.state = payload.status;
  }
  refreshLockButtonAvailability();
  if (payload.phase) {
    const phase = phaseTitles[payload.phase] || payload.phase;
    card.phase.dataset.translationKey = phase;
    card.phase.textContent = t(phase);
  }
  if (Number.isFinite(payload.completed) && Number.isFinite(payload.total) && payload.total > 0) {
    card.progress.max = payload.total;
    card.progress.value = Math.min(payload.completed, payload.total);
  } else if (payload.status === "running" && payload.phase !== "completed") {
    card.progress.removeAttribute("value");
  }
  if (["cancelling", "completed", "failed", "cancelled"].includes(payload.status)) {
    card.cancel.hidden = true;
    card.cancel.disabled = true;
  }
  if (["completed", "failed", "cancelled"].includes(payload.status)) {
    if (!card.progress.hasAttribute("value")) card.progress.hidden = true;
    if (payload.message) {
      const message = window.NexusQI18n.translateValue(payload.message);
      card.phase.dataset.translationKey = payload.message;
      card.phase.textContent = message;
      if (payload.status === "failed") showMessage(message, true);
    }
  }
}
window.__TAURI__.event.listen("operation-update", (event) => updateOperationCard(event.payload))
  .catch((error) => showMessage(String(error), true));

function showMessage(message, isError = false) {
  feedback.hidden = false;
  feedback.classList.toggle("error", isError);
  feedback.setAttribute("role", isError ? "alert" : "status");
  feedback.setAttribute("aria-live", isError ? "assertive" : "polite");
  feedback.textContent = window.NexusQI18n.translateValue(message);
}

function clearSecrets(form) {
  form.querySelectorAll(".secret-row").forEach((row) => {
    const input = row.querySelector("input");
    const toggle = row.querySelector("button");
    input.value = "";
    input.type = "password";
    input.setCustomValidity("");
    toggle.textContent = t("Show");
    toggle.setAttribute("aria-pressed", "false");
    toggle.setAttribute("aria-label", t("Show password"));
  });
}

function clearAllSecrets() {
  document.querySelectorAll(".secret-row").forEach((row) => {
    const input = row.querySelector("input");
    const toggle = row.querySelector("button");
    if (!input || !toggle) return;
    input.value = "";
    input.type = "password";
    input.setCustomValidity("");
    toggle.textContent = t("Show");
    toggle.setAttribute("aria-pressed", "false");
    toggle.setAttribute("aria-label", t("Show password"));
  });
}

function bindSecretToggle(buttonId, inputId) {
  const button = $(buttonId);
  const input = $(inputId);
  button.addEventListener("click", () => {
    const reveal = input.type === "password";
    input.type = reveal ? "text" : "password";
    button.textContent = reveal ? (t("Hide")) : (t("Show"));
    button.setAttribute("aria-pressed", String(reveal));
    button.setAttribute("aria-label", `${reveal ? (t("Hide")) : (t("Show"))} ${t("password")}`);
  });
}

function refreshLockButtonAvailability() {
  const hasActiveOperations = [...operationCards.values()].some(({ status }) =>
    status.dataset.state === "queued" || status.dataset.state === "running" || status.dataset.state === "cancelling");
  lockButton.disabled = !vaultUnlocked || hasActiveOperations;
}

async function refreshStatus() {
  try {
    const unlocked = await invoke("vault_status");
    vaultUnlocked = unlocked;
    statusLabel.textContent = t(unlocked ? "Unlocked" : "Locked");
    statusDot.classList.toggle("active", unlocked);
    refreshLockButtonAvailability();
    document.querySelectorAll(".workspace input,.workspace select,.workspace button")
      .forEach((el) => { el.disabled = !unlocked; });
    if (unlocked) await refreshSharingData();
  } catch (error) {
    showMessage(String(error), true);
  }
}

function submit(formId, command, fields) {
  const form = $(formId);
  let submittedOperationId = null;
  form.querySelectorAll("[data-confirm-for]").forEach((confirmation) => {
    const password = $(confirmation.dataset.confirmFor);
    [password, confirmation].forEach((input) => {
      input.addEventListener("input", () => confirmation.setCustomValidity(""));
    });
  });
  const handleConfirmedSubmit = passwordConfirmation.createConfirmedFormSubmit({
    readConfirmations: () => [...form.querySelectorAll("[data-confirm-for]")].map((confirmation) => ({
      input: confirmation,
      value: $(confirmation.dataset.confirmFor).value,
      confirmation: confirmation.value,
    })),
    onMismatch: ({ input }) => {
      const message = t("Passwords do not match. Check both fields.");
      input.setCustomValidity(message);
      input.reportValidity();
      input.focus();
      showMessage(message, true);
    },
    onMatch: () => {
      const payload = Object.fromEntries(fields.map(([key, id]) => [key, $(id).value]));
      if (trackedCommands.has(command)) {
        const operationId = crypto.randomUUID();
        submittedOperationId = operationId;
        createOperationCard(operationId, command);
        return invoke(command, { ...payload, operationId });
      }
      return invoke(command, payload);
    },
  });
  form.addEventListener("submit", async (event) => {
    const validation = handleConfirmedSubmit(event);
    if (!validation.matched) return;
    const confirmations = [...form.querySelectorAll("[data-confirm-for]")];
    confirmations.forEach((confirmation) => confirmation.setCustomValidity(""));
    const button = form.querySelector('button[type="submit"]');
    const originalLabel = button.textContent;
    button.disabled = true;
    button.textContent = t("Processing…");
    form.setAttribute("aria-busy", "true");
    showMessage(t("Operation in progress. Please wait."));
    try {
      showMessage(await validation.result);
    } catch (error) {
      if (submittedOperationId) {
        const card = operationCards.get(submittedOperationId);
        if (card && ["queued", "running"].includes(card.status.dataset.state)) {
          updateOperationCard({ operationId: submittedOperationId, status: "failed" });
        }
      }
      showMessage(String(error), true);
    } finally {
      clearSecrets(form);
      form.removeAttribute("aria-busy");
      button.textContent = originalLabel;
      button.disabled = false;
      await refreshStatus();
    }
  });
}

function childPath(parent, childName) {
  const isWindowsPath = /^[A-Za-z]:[\\/]/.test(parent) || parent.startsWith("\\\\");
  const separator = isWindowsPath ? "\\" : "/";
  return `${parent.replace(/[\\/]+$/, "")}${separator}${childName}`;
}

function bindPicker(buttonId, inputId, command, kind, transform = (path) => path) {
  const button = $(buttonId);
  button.addEventListener("click", async () => {
    const originalLabel = button.textContent;
    button.disabled = true;
    button.textContent = t("Opening…");
    try {
      const selectedPath = await invoke(command, { kind });
      if (selectedPath) {
        $(inputId).value = transform(selectedPath);
        showMessage(t("Path selected."));
      }
    } catch (error) {
      showMessage(String(error), true);
    } finally {
      button.textContent = originalLabel;
      button.disabled = false;
      await refreshStatus();
    }
  });
}

const SHARE_CONTACTS_KEY = "nexusq-share-contacts-v1";
let pendingContactCard = null;

function storedVerifiedContacts() {
  try {
    const parsed = JSON.parse(localStorage.getItem(SHARE_CONTACTS_KEY) || "[]");
    return Array.isArray(parsed) ? parsed.filter((item) => item?.verified === true && item.card?.fingerprint) : [];
  } catch { return []; }
}

function saveVerifiedContacts(contacts) {
  localStorage.setItem(SHARE_CONTACTS_KEY, JSON.stringify(contacts));
}

function refreshVerifiedContacts() {
  const contacts = storedVerifiedContacts();
  const select = $("share-recipient");
  const previous = select.value;
  select.replaceChildren();
  if (!contacts.length) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = t("Import and verify a contact first");
    select.append(option);
    $("share-contact-card-json").value = "";
  } else {
    for (const item of contacts) {
      const option = document.createElement("option");
      option.value = item.card.key_id;
      option.textContent = `${item.card.label} · ${item.card.fingerprint.slice(-16)}`;
      select.append(option);
    }
    select.value = contacts.some((item) => item.card.key_id === previous) ? previous : contacts[0].card.key_id;
    const selected = contacts.find((item) => item.card.key_id === select.value);
    $("share-contact-card-json").value = JSON.stringify(selected.card);
  }
  const list = $("verified-contacts-list");
  list.replaceChildren();
  if (!contacts.length) {
    list.textContent = t("No verified contacts imported.");
  } else {
    for (const item of contacts) {
      const entry = document.createElement("p");
      entry.textContent = `${item.card.label} · ${item.card.fingerprint}`;
      list.append(entry);
    }
  }
}

async function refreshSharingData(preferredKeyId = "") {
  refreshVerifiedContacts();
  try {
    const identities = await invoke("list_receiving_identities");
    const select = $("receiving-identity-select");
    const previous = preferredKeyId || select.value;
    select.replaceChildren();
    if (!identities.length) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = t("Create a receiving identity first");
      select.append(option);
    } else {
      for (const identity of identities) {
        const option = document.createElement("option");
        option.value = identity.key_id;
        const keyStatus = identity.status === "retired" ? t("Retired") : identity.status === "rotating" ? t("Rotating") : t("Active");
        option.textContent = `${identity.label} · ${keyStatus} · ${identity.fingerprint.slice(-16)}`;
        select.append(option);
      }
      select.value = identities.some((identity) => identity.key_id === previous) ? previous : identities[0].key_id;
    }
    const selectedIdentity = identities.find((identity) => identity.key_id === select.value);
    $("export-receiving-contact").disabled = !selectedIdentity || selectedIdentity.status !== "active" || !vaultUnlocked;
  } catch (error) {
    showMessage(String(error), true);
  }
}

$("create-receiving-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!vaultUnlocked) return showMessage(t("Unlock your vault first."), true);
  const button = event.currentTarget.querySelector('button[type="submit"]');
  const original = button.textContent;
  button.disabled = true;
  button.textContent = t("Processing…");
  try {
    const card = await invoke("create_receiving_identity", { label: $("receiving-label").value });
    $("receiving-card-summary").hidden = false;
    $("receiving-card-summary").textContent = `${t("Receiving identity created")}: ${card.label} · ${t("Fingerprint")}: ${card.fingerprint}. ${t("Export and keep this public card safe; back up your vault keys separately.")}`;
    await refreshSharingData(card.key_id);
    showMessage(t("Receiving identity created."));
  } catch (error) {
    showMessage(String(error), true);
    await refreshSharingData();
  } finally {
    button.textContent = original;
    button.disabled = !vaultUnlocked;
  }
});

$("receiving-identity-select")?.addEventListener("change", async () => {
  const identities = await invoke("list_receiving_identities").catch(() => []);
  const selected = identities.find((identity) => identity.key_id === $("receiving-identity-select").value);
  $("export-receiving-contact").disabled = !selected || selected.status !== "active" || !vaultUnlocked;
});

$("export-receiving-contact")?.addEventListener("click", async () => {
  const identityKeyId = $("receiving-identity-select").value;
  const outputPath = $("receiving-contact-export-path").value;
  if (!identityKeyId || !outputPath) return showMessage(t("Choose a receiving identity and contact card destination."), true);
  const identity = await invoke("list_receiving_identities").then((items) => items.find((item) => item.key_id === identityKeyId));
  if (!identity) return showMessage(t("The selected receiving identity is unavailable."), true);
  try {
    showMessage(await invoke("export_receiving_contact", { identityId: identity.identity_id, outputPath }));
  } catch (error) { showMessage(String(error), true); }
});

$("import-contact-button")?.addEventListener("click", async () => {
  const path = $("contact-import-path").value;
  if (!path) return showMessage(t("Select a contact card first."), true);
  try {
    pendingContactCard = await invoke("import_contact_card", { path });
    $("pending-contact-details").hidden = false;
    $("pending-contact-summary").textContent = `${t("Recipient")}: ${pendingContactCard.label} · ${t("Identity")}: ${pendingContactCard.identity_id} · ${t("Fingerprint")}: ${pendingContactCard.fingerprint}. ${t("Compare every fingerprint group with the recipient through a separate trusted channel. A valid card alone does not prove who sent it.")}`;
    $("verify-contact-checkbox").checked = false;
    $("save-contact-button").disabled = true;
    showMessage(t("Contact card read. Verify its fingerprint before saving."));
  } catch (error) { showMessage(String(error), true); }
});

$("verify-contact-checkbox")?.addEventListener("change", (event) => {
  $("save-contact-button").disabled = !event.target.checked || !pendingContactCard || !vaultUnlocked;
});

$("save-contact-button")?.addEventListener("click", () => {
  if (!pendingContactCard || !$("verify-contact-checkbox").checked) return;
  const contacts = storedVerifiedContacts().filter((item) => item.card.key_id !== pendingContactCard.key_id);
  contacts.push({ card: pendingContactCard, verified: true, verifiedAt: new Date().toISOString() });
  saveVerifiedContacts(contacts);
  pendingContactCard = null;
  $("pending-contact-details").hidden = true;
  $("verify-contact-checkbox").checked = false;
  refreshVerifiedContacts();
  showMessage(t("Verified contact saved on this device."));
});

$("share-recipient")?.addEventListener("change", () => {
  const selected = storedVerifiedContacts().find((item) => item.card.key_id === $("share-recipient").value);
  $("share-contact-card-json").value = selected ? JSON.stringify(selected.card) : "";
});

submit("share-file-form", "create_shared_package", [["inputPath", "share-source-path"], ["outputPath", "share-package-path"], ["contactCardJson", "share-contact-card-json"]]);
submit("open-shared-form", "open_shared_package", [["inputPath", "shared-package-input"], ["outputPath", "shared-output-path"], ["keyId", "receiving-identity-select"]]);

bindPicker("receiving-contact-export-picker", "receiving-contact-export-path", "pick_save_path", "contact");
bindPicker("contact-import-picker", "contact-import-path", "pick_file_path", "contact");
bindPicker("share-source-picker", "share-source-path", "pick_file_path", "any");
bindPicker("share-package-picker", "share-package-path", "pick_save_path", "shared");
bindPicker("shared-package-input-picker", "shared-package-input", "pick_file_path", "shared");
bindPicker("shared-output-picker", "shared-output-path", "pick_save_path", "restored");

submit("create-form", "create_vault", [["path", "create-path"], ["password", "create-password"], ["passwordConfirmation", "create-password-confirm"]]);
submit("open-form", "open_vault", [["path", "open-path"], ["password", "open-password"]]);
submit("encrypt-form", "encrypt_file", [["inputPath", "encrypt-path"]]);
submit("decrypt-form", "decrypt_file", [["inputPath", "decrypt-input"], ["outputPath", "decrypt-output"]]);
submit("encrypt-folder-form", "encrypt_folder", [["sourcePath", "encrypt-folder-source"], ["vaultFolderPath", "encrypt-folder-output"]]);
submit("decrypt-folder-form", "decrypt_folder", [["vaultFolderPath", "decrypt-folder-source"], ["destinationPath", "decrypt-folder-output"]]);
submit("backup-form", "export_backup", [["backupPath", "backup-path"], ["backupPassphrase", "backup-password"], ["backupPassphraseConfirmation", "backup-password-confirm"]]);
submit("restore-form", "import_backup", [["backupPath", "restore-backup-path"], ["backupPassphrase", "restore-backup-password"], ["newVaultPath", "restore-vault-path"], ["newVaultPassword", "restore-vault-password"], ["newVaultPasswordConfirmation", "restore-vault-password-confirm"]]);

bindSecretToggle("create-password-toggle", "create-password");
bindSecretToggle("create-password-confirm-toggle", "create-password-confirm");
bindSecretToggle("open-password-toggle", "open-password");
bindSecretToggle("backup-password-toggle", "backup-password");
bindSecretToggle("backup-password-confirm-toggle", "backup-password-confirm");
bindSecretToggle("restore-backup-password-toggle", "restore-backup-password");
bindSecretToggle("restore-vault-password-toggle", "restore-vault-password");
bindSecretToggle("restore-vault-password-confirm-toggle", "restore-vault-password-confirm");

bindPicker("create-path-picker", "create-path", "pick_save_path", "vault");
bindPicker("open-path-picker", "open-path", "pick_file_path", "vault");
bindPicker("encrypt-path-picker", "encrypt-path", "pick_file_path", "any");
bindPicker("decrypt-input-picker", "decrypt-input", "pick_file_path", "encrypted");
bindPicker("decrypt-output-picker", "decrypt-output", "pick_save_path", "restored");
bindPicker("encrypt-folder-source-picker", "encrypt-folder-source", "pick_folder_path", "source");
bindPicker("encrypt-folder-output-picker", "encrypt-folder-output", "pick_folder_path", "parent",
  (path) => childPath(path, "NEXUS-Q-FolderVault"));
bindPicker("decrypt-folder-source-picker", "decrypt-folder-source", "pick_folder_path", "vault-folder");
bindPicker("decrypt-folder-output-picker", "decrypt-folder-output", "pick_folder_path", "parent",
  (path) => childPath(path, "Restored-Folder"));
bindPicker("backup-path-picker", "backup-path", "pick_save_path", "backup");
bindPicker("restore-backup-path-picker", "restore-backup-path", "pick_file_path", "backup");
bindPicker("restore-vault-path-picker", "restore-vault-path", "pick_save_path", "vault");

lockButton.addEventListener("click", async () => {
  const originalLabel = lockButton.textContent;
  lockButton.disabled = true;
  lockButton.textContent = t("Locking…");
  try {
    showMessage(await invoke("lock_vault"));
  } catch (error) {
    showMessage(String(error), true);
  } finally {
    clearAllSecrets();
    lockButton.textContent = originalLabel;
    await refreshStatus();
  }
});

refreshStatus();

// Local preferences: language and profile never leave this device.
const preference = (key, fallback = "") => { try { return localStorage.getItem(`nqv.${key}`) ?? fallback; } catch { return fallback; } };
const storePreference = (key, value) => { try { localStorage.setItem(`nqv.${key}`, value); } catch { /* storage may be unavailable */ } };
let lastActivitySent = 0;
function reportActivity() {
  const now = Date.now();
  if (now - lastActivitySent < 10000) return;
  lastActivitySent = now;
  invoke("record_activity").catch(() => {});
}
["pointerdown", "pointermove", "keydown", "touchstart", "wheel"].forEach((eventName) => {
  document.addEventListener(eventName, reportActivity, { passive: true });
});
document.addEventListener("visibilitychange", async () => {
  if (document.hidden) {
    try {
      await invoke("lock_vault");
    } catch (error) {
      if (!String(error).includes("No hay ningún vault desbloqueado")) showMessage(String(error), true);
    } finally {
      clearAllSecrets();
      await refreshStatus();
    }
  } else {
    await refreshStatus();
  }
});
const idleTimeoutSelect = $("idle-timeout");
async function initializeIdleTimeout() {
  if (!idleTimeoutSelect) return;
  try {
    const stored = Number(preference("idleTimeout", "300"));
    const allowed = [0, 60, 300, 600, 900, 1800];
    const seconds = allowed.includes(stored) ? stored : 300;
    await invoke("set_idle_timeout", { seconds });
    idleTimeoutSelect.value = String(seconds);
  } catch (error) { showMessage(String(error), true); }
}
idleTimeoutSelect?.addEventListener("change", async () => {
  const seconds = Number(idleTimeoutSelect.value);
  idleTimeoutSelect.disabled = true;
  try {
    await invoke("set_idle_timeout", { seconds });
    storePreference("idleTimeout", String(seconds));
    showMessage(t("Automatic lock updated."));
  } catch (error) { showMessage(String(error), true); }
  finally { idleTimeoutSelect.disabled = false; }
});
window.__TAURI__.event.listen("vault-auto-locked", async () => {
  clearAllSecrets();
  await refreshStatus();
  showMessage(t("The vault was automatically locked for security."));
}).catch((error) => showMessage(String(error), true));
window.__TAURI__.event.listen("vault-lock-failed", (event) => {
  clearAllSecrets();
  showMessage(String(event.payload), true);
  refreshStatus();
}).catch((error) => showMessage(String(error), true));
initializeIdleTimeout();

const themeToggle = $("theme-toggle");
async function translatePage(language) {
  await window.NexusQI18n.setLanguage(language);
  const selector = $("language-select");
  if (selector) selector.value = window.NexusQI18n.language;
  storePreference("language", window.NexusQI18n.language);
  if (themeToggle) applyTheme(preference("theme", "light"));
  for (const card of operationCards.values()) {
    card.title.textContent = t(operationTitles[card.command] || "Vault operation");
    card.context.textContent = t(operationContexts[card.command] || "Local vault operation");
    card.status.textContent = t(({ queued: "Queued", running: "Running", cancelling: "Cancelling", completed: "Completed", failed: "Failed", cancelled: "Cancelled" })[card.status.dataset.state] || card.status.dataset.state);
    if (card.phase.dataset.translationKey) card.phase.textContent = window.NexusQI18n.translateValue(card.phase.dataset.translationKey);
    card.cancel.textContent = t(card.cancel.dataset.translationKey || "Cancel");
    card.progress.setAttribute("aria-label", t("Operation progress"));
  }
}
const languageSelect = $("language-select");
if (languageSelect) {
  languageSelect.addEventListener("change", () => translatePage(languageSelect.value));
  window.NexusQI18n.ready.then(() => translatePage(preference("language", "en")));
}
function applyTheme(theme) {
  const dark = theme === "dark";
  document.body.classList.toggle("dark", dark);
  if (themeToggle) {
    themeToggle.textContent = dark ? t("Light mode") : t("Dark mode");
    themeToggle.setAttribute("aria-label", dark ? t("Switch to light mode") : t("Switch to dark mode"));
  }
  storePreference("theme", theme);
}
applyTheme(preference("theme", "light"));
themeToggle?.addEventListener("click", () => applyTheme(document.body.classList.contains("dark") ? "light" : "dark"));
function applyProfile(name, photo) {
  const safeName = name || "Local user";
  $("sidebar-user").textContent = safeName;
  const initial = safeName.trim().charAt(0).toUpperCase() || "U";
  [$("sidebar-avatar"), $("profile-preview")].forEach((avatar) => {
    if (!avatar) return;
    avatar.textContent = "";
    if (photo) { const img = document.createElement("img"); img.src = photo; img.alt = "Profile"; avatar.append(img); }
    else avatar.textContent = initial;
  });
}
let profilePhoto = preference("profilePhoto");
$("profile-name").value = preference("profileName");
applyProfile(preference("profileName"), profilePhoto);
$("profile-photo")?.addEventListener("change", (event) => {
  const file = event.target.files?.[0];
  if (!file) return;
  if (file.size > 2 * 1024 * 1024) { showMessage(t("Choose an image smaller than 2 MiB."), true); event.target.value = ""; return; }
  const reader = new FileReader();
  reader.onload = () => { profilePhoto = String(reader.result); applyProfile($("profile-name").value, profilePhoto); };
  reader.readAsDataURL(file);
});
$("save-profile")?.addEventListener("click", () => {
  const name = $("profile-name").value.trim().slice(0, 40);
  storePreference("profileName", name); storePreference("profilePhoto", profilePhoto);
  applyProfile(name, profilePhoto); showMessage(t("Local profile saved."));
});
// Sidebar navigation switches independent views instead of scrolling one long page.
const pageLinks = [...document.querySelectorAll(".side-nav a")];
const pages = [...document.querySelectorAll(".app-page")];
function showPage(pageId, updateHash = true) {
  const selected = pages.some((page) => page.id === pageId) ? pageId : "overview";
  pages.forEach((page) => page.classList.toggle("active", page.id === selected));
  pageLinks.forEach((link) => {
    const active = link.getAttribute("href") === "#" + selected;
    link.classList.toggle("active", active);
    if (active) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  });
  if (updateHash) history.replaceState(null, "", "#" + selected);
  window.scrollTo({ top: 0, behavior: "instant" });
}
document.querySelectorAll("[data-go-page]").forEach((link) => link.addEventListener("click", (event) => { event.preventDefault(); showPage(link.dataset.goPage); }));
pageLinks.forEach((link) => link.addEventListener("click", (event) => {
  event.preventDefault();
  showPage(link.hash.slice(1));
}));
showPage(location.hash.slice(1) || "overview", false);

document.querySelector("[data-open-repository]")?.addEventListener("click", async (event) => {
  event.preventDefault();
  try {
    await window.__TAURI__.core.invoke("open_project_repository");
  } catch (error) {
    showMessage("Could not open GitHub: " + error, true);
  }
});

// Generate a high-entropy password locally using the Web Crypto API.
$("generate-password")?.addEventListener("click", () => {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*_-+=?";
  const limit = Math.floor(256 / alphabet.length) * alphabet.length;
  let result = "";
  while (result.length < 24) {
    const bytes = crypto.getRandomValues(new Uint8Array(32));
    for (const byte of bytes) if (byte < limit && result.length < 24) result += alphabet[byte % alphabet.length];
  }
  const input = $("create-password");
  input.value = result;
  input.type = "text";
  $("create-password-toggle").textContent = t("Hide");
  $("create-password-toggle").setAttribute("aria-pressed", "true");
  showMessage(t("A strong password was generated locally. Save it in a password manager before creating the vault."));
});
