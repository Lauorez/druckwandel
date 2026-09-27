import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

export const SETTINGS_WINDOW_LABEL = "settings";
export const SETTINGS_CHANGED_EVENT = "app-settings-changed";
export const SETTINGS_NAVIGATE_EVENT = "settings-navigate";

export type SettingsSection = "profiles" | "archive" | "datev" | "backup" | "help";
export type SettingsScope = SettingsSection | "learning";

export function settingsHash(section?: SettingsSection): string {
  return section && section !== "profiles" ? `#settings/${section}` : "#settings";
}

export function isSettingsRoute(hash = window.location.hash): boolean {
  return hash === "#settings" || hash.startsWith("#settings/");
}

export function settingsSectionFromHash(hash = window.location.hash): SettingsSection {
  const match = hash.match(/^#settings\/(profiles|archive|datev|backup|help)\b/);
  return match?.[1] as SettingsSection | undefined ?? "profiles";
}

export async function emitSettingsChanged(scope: SettingsScope): Promise<void> {
  if (!isTauri()) {
    window.dispatchEvent(new CustomEvent(SETTINGS_CHANGED_EVENT, { detail: { scope } }));
    return;
  }
  try {
    await emit(SETTINGS_CHANGED_EVENT, { scope });
  } catch {
    /* Tests and browser builds have no event IPC. */
  }
}

export async function listenSettingsChanged(handler: (scope: SettingsScope) => void): Promise<() => void> {
  const onWindow = (event: Event) => {
    const scope = (event as CustomEvent<{ scope?: SettingsScope }>).detail?.scope;
    if (scope) handler(scope);
  };
  window.addEventListener(SETTINGS_CHANGED_EVENT, onWindow);
  if (!isTauri()) return () => window.removeEventListener(SETTINGS_CHANGED_EVENT, onWindow);
  try {
    const unlisten = await listen<{ scope: SettingsScope }>(SETTINGS_CHANGED_EVENT, (event) => {
      if (event.payload?.scope) handler(event.payload.scope);
    });
    return () => {
      window.removeEventListener(SETTINGS_CHANGED_EVENT, onWindow);
      unlisten();
    };
  } catch {
    return () => window.removeEventListener(SETTINGS_CHANGED_EVENT, onWindow);
  }
}

export async function listenSettingsNavigate(handler: (section: SettingsSection) => void): Promise<() => void> {
  const onHash = () => handler(settingsSectionFromHash());
  window.addEventListener("hashchange", onHash);
  if (!isTauri()) return () => window.removeEventListener("hashchange", onHash);
  try {
    const unlisten = await listen<{ section?: SettingsSection }>(SETTINGS_NAVIGATE_EVENT, (event) => {
      handler(event.payload?.section ?? "profiles");
    });
    return () => {
      window.removeEventListener("hashchange", onHash);
      unlisten();
    };
  } catch {
    return () => window.removeEventListener("hashchange", onHash);
  }
}

export async function openSettingsWindow(section: SettingsSection = "profiles"): Promise<"window" | "fallback"> {
  if (!isTauri()) return "fallback";
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const existing = await WebviewWindow.getByLabel(SETTINGS_WINDOW_LABEL);
    if (existing) {
      const { Window } = await import("@tauri-apps/api/window");
      const win = await Window.getByLabel(SETTINGS_WINDOW_LABEL);
      if (!win) return "fallback";
      await win.show();
      await win.unminimize();
      await win.setFocus();
      await emit(SETTINGS_NAVIGATE_EVENT, { section });
      return "window";
    }
    const webview = new WebviewWindow(SETTINGS_WINDOW_LABEL, {
      url: `index.html${settingsHash(section)}`,
      title: "Einstellungen",
      width: 820,
      height: 880,
      minWidth: 640,
      minHeight: 520,
      center: true,
      focus: true,
      resizable: true,
    });
    await new Promise<void>((resolve, reject) => {
      void webview.once("tauri://created", () => resolve()).catch(reject);
      void webview.once("tauri://error", (event: { payload: unknown }) => reject(event.payload ?? "Einstellungen konnten nicht geöffnet werden.")).catch(reject);
    });
    return "window";
  } catch {
    return "fallback";
  }
}

export async function closeSettingsWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().destroy();
}
