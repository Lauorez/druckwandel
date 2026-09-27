import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { canonicalJson } from "../../../src/export/invoice-snapshot.js";
import { validateDatevProfile } from "../../../src/export/datev/export.js";
import {
  emptyCorrectionMemory,
  listLearnedAssignments,
  removeLearnedAssignment,
  setLearnedAssignmentEnabled,
} from "../../../src/learning/correction-memory.js";
import {
  activeLearningProfile,
  createLearningProfile,
  deleteLearningProfile,
  emptyLearningProfileStore,
  renameLearningProfile,
  replaceActiveMemory,
  selectLearningProfile,
  undoActiveMemory,
  type LearningProfileStore,
} from "../../../src/learning/profiles.js";
import { getArchiveStatus, setArchiveSigning, type ArchiveStatus } from "./archiveStore.js";
import {
  backupStatus,
  confirmRestore,
  createBackup,
  formatBackupTime,
  previewRestore,
  resumeRestore,
  type BackupStatus,
  type RestorePreview,
} from "./backupStore.js";
import { diagnosticPreview, loadDiagnosticReport, saveDiagnosticReport, type DiagnosticReport } from "./diagnoseStore.js";
import { datevStore, emptyDatevProfile, parseStoredDatevProfile } from "./datevStore.js";
import { DatevProfileForm } from "./DatevProfileForm.js";
import { LearningProfilesSettings } from "./LearningProfilesSettings.js";
import { loadLearningProfiles, saveLearningProfiles } from "./learningMemoryStore.js";
import { subscription } from "./subscription.js";
import {
  closeSettingsWindow,
  listenSettingsChanged,
  listenSettingsNavigate,
  settingsSectionFromHash,
  type SettingsSection,
} from "./settingsWindow.js";

const describe = (error: unknown) => error instanceof Error ? error.message : String(error);

interface SettingsViewProps {
  overlay?: boolean;
  initialSection?: SettingsSection;
  onClose?: () => void;
}

export function SettingsView({ overlay = false, initialSection, onClose }: SettingsViewProps) {
  const [section, setSection] = useState<SettingsSection>(initialSection ?? settingsSectionFromHash());
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const pendingSave = useRef<(() => Promise<void>) | null>(null);
  const leaving = useRef(false);
  const dialog = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!overlay) return;
    const previous = document.activeElement;
    dialog.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [overlay]);

  async function leave(action: () => void | Promise<void>) {
    if (leaving.current) return;
    leaving.current = true;
    try {
      await pendingSave.current?.();
      await action();
    } catch (reason) {
      setError(`Einstellungen nicht gespeichert: ${describe(reason)}`);
    } finally { leaving.current = false; }
  }
  const navigate = (next: SettingsSection) => leave(() => { setSection(next); setMessage(""); });
  const actions = useRef({ close, navigate, leave });
  actions.current = { close, navigate, leave };

  useEffect(() => {
    if (initialSection) void actions.current.navigate(initialSection);
  }, [initialSection]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void actions.current.close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    return subscription(listenSettingsNavigate((next) => { void actions.current.navigate(next); }));
  }, []);

  useEffect(() => {
    if (overlay || !isTauri()) return;
    return subscription(import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      return win.onCloseRequested((event) => {
        event.preventDefault();
        void actions.current.leave(() => win.destroy());
      });
    }));
  }, [overlay]);

  async function close() {
    await leave(async () => {
      if (!overlay) await closeSettingsWindow();
      onClose?.();
    });
  }

  return <div className={overlay ? "dialog-backdrop" : undefined} onMouseDown={(event) => {
    if (overlay && event.target === event.currentTarget) void close();
  }}>
    <main
      ref={dialog}
      className={`settings-main${overlay ? " settings-overlay" : ""}`}
      role={overlay ? "dialog" : undefined}
      aria-modal={overlay || undefined}
      aria-labelledby="settings-title"
      onKeyDown={(event) => {
        if (!overlay || event.key !== "Tab") return;
        const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]',
        ));
        const first = controls[0], last = controls.at(-1);
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }}
    >
      <header>
        <div>
          <h1 id="settings-title">Einstellungen</h1>
          <p>Dauerhafte Angaben für diesen Computer</p>
        </div>
        <button type="button" className="secondary" onClick={() => void close()}>Schließen</button>
      </header>
      <nav className="settings-nav" aria-label="Einstellungsbereich">
        <button type="button" className={section === "profiles" ? "active" : ""} onClick={() => void navigate("profiles")}>Erkennungsprofile</button>
        {isTauri() && <button type="button" className={section === "archive" ? "active" : ""} onClick={() => void navigate("archive")}>Archivschutz</button>}
        {isTauri() && <button type="button" className={section === "backup" ? "active" : ""} onClick={() => void navigate("backup")}>Sicherung</button>}
        {isTauri() && <button type="button" className={section === "help" ? "active" : ""} onClick={() => void navigate("help")}>Diagnose</button>}
        {isTauri() && <button type="button" className={section === "datev" ? "active" : ""} onClick={() => void navigate("datev")}>Steuerkanzlei</button>}
      </nav>
      {error && <div className="error-banner" role="alert">{error}<button type="button" onClick={() => setError("")} aria-label="Meldung schließen">×</button></div>}
      {message && <div className="datev-feedback" role="status">{message}</div>}
      <div className="settings-body">
        {section === "profiles" && <LearningSettings pendingSave={pendingSave} onMessage={setMessage} onError={setError} />}
        {section === "archive" && isTauri() && <ArchiveSettings onError={setError} />}
        {section === "backup" && isTauri() && <BackupSettings onMessage={setMessage} onError={setError} />}
        {section === "help" && isTauri() && <DiagnosticSettings onMessage={setMessage} onError={setError} />}
        {section === "datev" && isTauri() && <DatevSettings pendingSave={pendingSave} onMessage={setMessage} onError={setError} />}
      </div>
    </main>
  </div>;
}

function LearningSettings({ pendingSave, onMessage, onError }: { pendingSave: RefObject<(() => Promise<void>) | null>; onMessage: (value: string) => void; onError: (value: string) => void }) {
  const [store, setStore] = useState<LearningProfileStore>(emptyLearningProfileStore);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const ready = useRef(emptyLearningProfileStore());
  const writer = useRef(Promise.resolve());

  function remember(next: LearningProfileStore) {
    ready.current = next;
    setStore(next);
    setLoaded(true);
  }

  async function reload() {
    const next = await loadLearningProfiles();
    remember(next);
    return next;
  }

  useEffect(() => {
    void reload().catch((reason) => onError(describe(reason)));
    return subscription(listenSettingsChanged((scope) => {
      if (scope === "learning" || scope === "profiles") void reload().catch((reason) => onError(describe(reason)));
    }));
  }, [onError]);

  async function persist(next: LearningProfileStore | null, success: string, failure: string) {
    if (!next || !loaded || busy) return;
    setBusy(true);
    const task = (async () => {
      await saveLearningProfiles(next);
      remember(next);
      if (success) onMessage(success);
      onError("");
    })();
    writer.current = task.catch(() => undefined);
    try {
      await task;
    } catch (reason) {
      console.error(reason);
      onError(failure);
    } finally { setBusy(false); }
  }

  pendingSave.current = async () => { await writer.current; };
  useEffect(() => () => { pendingSave.current = null; }, [pendingSave]);

  const active = activeLearningProfile(store);
  return <section className="settings-section">
    <h2>Erkennungsprofile</h2>
    <fieldset className="learning-settings-controls" disabled={!loaded || busy}>
    <LearningProfilesSettings
      profiles={store.profiles.map((profile) => ({ id: profile.id, name: profile.name }))}
      activeProfileId={store.activeProfileId}
      learningRuleCount={active.memory.rules.length + active.memory.tableRules.length}
      canUndo={Boolean(active.undoMemory)}
      assignments={listLearnedAssignments(active.memory)}
      onSelectProfile={(profileId) => void persist(selectLearningProfile(ready.current, profileId), "", "Das Erkennungsprofil konnte nicht gewechselt werden.")}
      onCreateProfile={() => {
        const name = window.prompt("Name des neuen Erkennungsprofils", "Neues Profil")?.trim();
        if (!name) return;
        void persist(createLearningProfile(ready.current, name), `Erkennungsprofil „${name}“ wurde angelegt und ist ausgewählt. Es gilt ab der nächsten Rechnung.`, "Das Erkennungsprofil konnte nicht angelegt werden.");
      }}
      onRenameProfile={() => {
        const current = activeLearningProfile(ready.current);
        const name = window.prompt("Erkennungsprofil umbenennen", current.name)?.trim();
        if (!name) return;
        void persist(renameLearningProfile(ready.current, current.id, name), `Das Erkennungsprofil heißt jetzt „${name}“.`, "Das Erkennungsprofil konnte nicht umbenannt werden.");
      }}
      onDeleteProfile={() => {
        if (ready.current.profiles.length < 2) return;
        const current = activeLearningProfile(ready.current);
        if (!window.confirm(`Soll das Erkennungsprofil „${current.name}“ gelöscht werden? Die gemerkten Stellen dieses Profils gehen verloren.`)) return;
        void persist(deleteLearningProfile(ready.current, current.id), `Erkennungsprofil „${current.name}“ wurde gelöscht.`, "Das Erkennungsprofil konnte nicht gelöscht werden.");
      }}
      onClearMemory={() => {
        if (!window.confirm("Sollen die gemerkten Ergänzungen des aktuellen Profils gelöscht werden? Bereits geöffnete Rechnungen bleiben unverändert.")) return;
        void persist(replaceActiveMemory(ready.current, emptyCorrectionMemory()), "Die gemerkten Ergänzungen dieses Profils wurden gelöscht. Dies gilt ab der nächsten Rechnung.", "Die gemerkten Ergänzungen konnten nicht gelöscht werden.");
      }}
      onUndo={() => void persist(undoActiveMemory(ready.current), "Die letzte Bestätigung der gemerkten Stellen wurde rückgängig gemacht. Dies gilt ab der nächsten Rechnung.", "Die letzte Bestätigung konnte nicht rückgängig gemacht werden.")}
      onToggleAssignment={(id, enabled) => {
        const next = replaceActiveMemory(ready.current, setLearnedAssignmentEnabled(activeLearningProfile(ready.current).memory, id, enabled));
        void persist(next, enabled ? "Die gemerkte Stelle ist wieder aktiv. Dies gilt ab der nächsten Rechnung." : "Die gemerkte Stelle ist deaktiviert. Dies gilt ab der nächsten Rechnung.", "Die gemerkte Stelle konnte nicht geändert werden.");
      }}
      onRemoveAssignment={(id) => {
        if (!window.confirm("Soll diese gemerkte Stelle entfernt werden?")) return;
        void persist(replaceActiveMemory(ready.current, removeLearnedAssignment(activeLearningProfile(ready.current).memory, id)), "Die gemerkte Stelle wurde entfernt. Dies gilt ab der nächsten Rechnung.", "Die gemerkte Stelle konnte nicht entfernt werden.");
      }}
    />
    </fieldset>
  </section>;
}

function ArchiveSettings({ onError }: { onError: (value: string) => void }) {
  const [status, setStatus] = useState<ArchiveStatus>();
  const [busy, setBusy] = useState(false);

  async function reload() {
    setStatus(await getArchiveStatus());
  }

  useEffect(() => {
    void reload().catch((reason) => onError(describe(reason)));
  }, [onError]);

  async function changeProtection(enabled: boolean) {
    setBusy(true);
    try {
      setStatus(await setArchiveSigning(enabled));
      onError("");
    } catch (reason) {
      console.error(reason);
      onError("Der zusätzliche Schutz konnte nicht geändert werden.");
    } finally {
      setBusy(false);
    }
  }

  return <section className="settings-section">
    <h2>Zusätzlicher Schutz für neue Rechnungen</h2>
    <div className="archive-protection">
      <div>
        <strong>Neue Einträge bestätigen</strong>
        <span>Neue Archiveinträge können mit einem nur auf diesem Computer gespeicherten Schlüssel bestätigt werden. Bereits archivierte Rechnungen bleiben unverändert.</span>
      </div>
      <label className="switch-label">
        <input
          type="checkbox"
          checked={status?.signingEnabled ?? false}
          disabled={!status || busy}
          onChange={(event) => void changeProtection(event.target.checked)}
        />
        <span>{status?.signingEnabled ? "Eingeschaltet" : "Ausgeschaltet"}</span>
      </label>
    </div>
  </section>;
}

function BackupSettings({ onMessage, onError }: { onMessage: (value: string) => void; onError: (value: string) => void }) {
  const [status, setStatus] = useState<BackupStatus>();
  const [destination, setDestination] = useState("");
  const [password, setPassword] = useState("");
  const [passwordRepeat, setPasswordRepeat] = useState("");
  const [restorePath, setRestorePath] = useState("");
  const [restorePassword, setRestorePassword] = useState("");
  const [preview, setPreview] = useState<RestorePreview>();
  const [busy, setBusy] = useState(false);

  async function reload(includePreview = false) {
    const next = await backupStatus();
    setStatus(next);
    setDestination((current) => current || next.defaultPath);
    if (includePreview) setPreview(next.pendingRestore ? undefined : next.checkedPreview ?? undefined);
  }

  useEffect(() => {
    void reload(true).catch((reason) => onError(describe(reason)));
  }, [onError]);

  async function run(action: () => Promise<string>) {
    if (busy) return;
    setBusy(true);
    try {
      const message = await action();
      onError("");
      onMessage(message);
      await reload();
    } catch (reason) {
      onError(describe(reason));
    } finally {
      setBusy(false);
    }
  }

  return <section className="settings-section">
    <h2>Sicherung und Wiederherstellung</h2>
    <p>Archiv, Entwürfe, Originaldateien, Vorlagengedächtnis, Kanzleiangaben und der Archivschlüssel werden in eine passwortgeschützte Datei geschrieben. Eine Kopie auf derselben Festplatte schützt nicht vor einem Plattenausfall.</p>
    <p>Letzte Sicherung: {formatBackupTime(status?.lastBackupAtMs)}{status?.lastPath ? ` · ${status.lastPath}` : ""}</p>
    {status?.sameVolume && status.lastPath ? <p className="datev-notice" role="note">Die letzte Sicherung liegt auf demselben Datenträger wie die Arbeitsdaten.</p> : null}
    {status?.pendingRestore ? <div className="datev-notice" role="status">
      <p>Eine Wiederherstellung wurde unterbrochen. Bitte zuerst fortsetzen. Vorherige Daten und Angaben, die danach neu entstanden sind, bleiben gesondert erhalten.</p>
      <button type="button" disabled={busy} onClick={() => void run(async () => {
        const result = await resumeRestore();
        setPreview(undefined);
        return `Die unterbrochene Wiederherstellung wurde fortgesetzt (${result.archiveEntries} Archiveinträge).`;
      })}>Wiederherstellung fortsetzen</button>
    </div> : null}
    <fieldset className="backup-fields" disabled={busy}>
      <legend>Sicherung erstellen</legend>
      <label>Zieldatei<input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} /></label>
      <label>Sicherungskennwort<input type="password" autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>
      <label>Sicherungskennwort wiederholen<input type="password" autoComplete="new-password" value={passwordRepeat} onChange={(event) => setPasswordRepeat(event.target.value)} /></label>
      <button type="button" onClick={() => void run(async () => {
        if (password !== passwordRepeat) throw new Error("Die Kennwörter stimmen nicht überein.");
        const result = await createBackup(destination, password);
        setPassword("");
        setPasswordRepeat("");
        return `Sicherung gespeichert (${result.archiveEntries} Archiveinträge, ${result.drafts} Entwürfe).${result.sameVolume ? " Sie liegt auf demselben Datenträger wie die Arbeitsdaten und schützt nicht vor einem Plattenausfall." : ""}`;
      })}>Sicherung erstellen</button>
    </fieldset>
    <fieldset className="backup-fields" disabled={busy}>
      <legend>Wiederherstellen</legend>
      <p>Die Sicherung wird zuerst in einen isolierten Ordner geprüft. Erst nach Bestätigung ersetzen die Daten den aktuellen Bestand; der bisherige Stand bleibt unter dem Anwendungsordner wiederherstellbar.</p>
      <label>Sicherungsdatei<input value={restorePath} onChange={(event) => { setRestorePath(event.target.value); setPreview(undefined); }} spellCheck={false} /></label>
      <label>Kennwort der Sicherung<input type="password" autoComplete="current-password" value={restorePassword} onChange={(event) => { setRestorePassword(event.target.value); setPreview(undefined); }} /></label>
      <button type="button" onClick={() => void run(async () => {
        setPreview(undefined);
        const next = await previewRestore(restorePath, restorePassword);
        setPreview(next);
        return `Sicherung geprüft: ${next.archiveEntries} Archiveinträge, ${next.drafts} Entwürfe, ${next.originals} Originale. Bitte die Übernahme bestätigen.`;
      })}>Sicherung prüfen</button>
      {preview && <>
        <p>Stand der geprüften Sicherung: {formatBackupTime(preview.createdAtMs)}</p>
        <p>Geprüft: {preview.archiveEntries} Archiveinträge, {preview.drafts} Entwürfe, {preview.originals} Originale, {preview.datevExports} Kanzleiexporte{preview.hasSigningKey ? ", mit Archivschlüssel" : ""}.</p>
        <button type="button" onClick={() => {
          if (!window.confirm("Soll der aktuelle Datenbestand durch diese Sicherung ersetzt werden? Der bisherige Stand bleibt lokal erhalten und wird nicht automatisch gelöscht.")) return;
          void run(async () => {
            const result = await confirmRestore();
            setPreview(undefined);
            setRestorePassword("");
            return `Wiederherstellung abgeschlossen (${result.archiveEntries} Archiveinträge). Bitte die Anwendung anschließend neu öffnen.`;
          });
        }}>Geprüfte Sicherung übernehmen</button>
      </>}
    </fieldset>
  </section>;
}

function DiagnosticSettings({ onMessage, onError }: { onMessage: (value: string) => void; onError: (value: string) => void }) {
  const [report, setReport] = useState<DiagnosticReport>();
  const [destination, setDestination] = useState("");
  const [reviewed, setReviewed] = useState(false);
  const [busy, setBusy] = useState(false);
  const preview = report ? diagnosticPreview(report) : "";

  useEffect(() => {
    void loadDiagnosticReport().then(setReport).catch((reason) => onError(describe(reason)));
  }, [onError]);

  return <section className="settings-section">
    <h2>Diagnosebericht</h2>
    <p>Der Bericht enthält Versionen, Prüferstatus und Zähler. Keine PDF-, XML-, Bank- oder Rechnungsinhalte und keine persönlichen Dateipfade. Er wird nicht automatisch versendet.</p>
    <pre className="diagnose-preview" tabIndex={0} aria-label="Vorschau des Diagnoseberichts">{preview || "Bericht wird gelesen …"}</pre>
    <label className="switch-label">
      <input type="checkbox" checked={reviewed} onChange={(event) => setReviewed(event.target.checked)} />
      <span>Ich habe die Vorschau geprüft und möchte den Bericht bewusst speichern.</span>
    </label>
    <fieldset className="backup-fields" disabled={busy || !report}>
      <legend>Speichern</legend>
      <label>Zieldatei<input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} placeholder="D:\\extern\\diagnose.json" /></label>
      <button type="button" disabled={!reviewed || !destination.trim()} onClick={() => {
        if (!report || busy) return;
        setBusy(true);
        void saveDiagnosticReport(destination).then((path) => {
          onError("");
          onMessage(`Diagnosebericht gespeichert: ${path}`);
        }).catch((reason) => onError(describe(reason))).finally(() => setBusy(false));
      }}>Bericht speichern</button>
    </fieldset>
  </section>;
}

function DatevSettings({ pendingSave, onMessage, onError }: { pendingSave: RefObject<(() => Promise<void>) | null>; onMessage: (value: string) => void; onError: (value: string) => void }) {
  const [profile, setProfile] = useState(emptyDatevProfile);
  const profileJson = useMemo(() => canonicalJson(profile), [profile]);
  const [savedProfile, setSavedProfile] = useState(profileJson);
  const [ready, setReady] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const writer = useRef(Promise.resolve());
  const alive = useRef(true);
  const dirty = profileJson !== savedProfile;
  const current = useRef({ ready, contents: profileJson, saved: savedProfile });
  current.current = { ready, contents: profileJson, saved: savedProfile };
  const problems = useMemo(() => validateDatevProfile(profile), [profile]);

  async function reload() {
    const raw = await datevStore.profile();
    const next = parseStoredDatevProfile(raw);
    setProfile(next);
    setSavedProfile(canonicalJson(next));
    setReady(true);
  }

  function save(contents = profileJson) {
    setSaveFailed(false);
    const next = writer.current.catch(() => undefined).then(async () => {
      if (contents === current.current.saved) return;
      await datevStore.saveProfile(contents);
      current.current.saved = contents;
    });
    writer.current = next;
    void next.then(() => {
      if (alive.current) setSavedProfile(contents);
    }).catch((reason) => {
      if (alive.current) { setSaveFailed(true); onError(`Kanzleiangaben nicht gespeichert: ${describe(reason)}`); }
    });
    return next;
  }

  pendingSave.current = async () => {
    await writer.current.catch(() => undefined);
    while (current.current.ready && current.current.contents !== current.current.saved) {
      await save(current.current.contents);
    }
  };

  useEffect(() => () => { pendingSave.current = null; }, [pendingSave]);

  useEffect(() => {
    alive.current = true;
    void reload().catch((reason) => onError(describe(reason)));
    return () => { alive.current = false; };
  }, [onError]);

  useEffect(() => {
    if (!ready || !dirty) return;
    const timer = setTimeout(() => { void save(profileJson); }, 450);
    return () => clearTimeout(timer);
  }, [ready, dirty, profileJson]);

  return <section className="settings-section datev-view">
    <h2>Angaben der Steuerkanzlei</h2>
    <p>Bitte gemeinsam mit Ihrer Steuerkanzlei einrichten. Kontonummern werden nicht aus Rechnungen geraten. Diese erste Version unterstützt normale deutsche Ausgangsrechnungen in EUR mit 7 % oder 19 % Umsatzsteuer und Sollversteuerung.</p>
    <p className="settings-save-status" role="status">{saveFailed ? "Nicht gespeichert." : dirty ? "Wird gespeichert …" : ready ? "Gespeichert." : "Wird geladen …"}</p>
    {problems.length > 0 && <div className="datev-notice"><strong>Vor dem Export noch zu klären:</strong><ul>{problems.map((problem) => <li key={problem}>{problem}</li>)}</ul></div>}
    <DatevProfileForm
      profile={profile}
      onChange={setProfile}
      onSave={() => { void save().then(() => onMessage("Gespeichert.")).catch(() => undefined); }}
      busy={!ready}
      dirty={dirty}
      embedded
    />
  </section>;
}
