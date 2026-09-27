import { invoke } from "@tauri-apps/api/core";
import { emitSettingsChanged } from "./settingsWindow.js";

export interface BackupStatus {
  lastBackupAtMs?: number | null;
  lastPath?: string | null;
  sameVolume: boolean;
  reminderDue: boolean;
  pendingRestore: boolean;
  pendingPreview?: boolean;
  checkedPreview?: RestorePreview | null;
  defaultPath: string;
}

export interface BackupResult {
  path: string;
  createdAtMs: number;
  sameVolume: boolean;
  archiveEntries: number;
  drafts: number;
}

export interface RestorePreview {
  archiveEntries: number;
  drafts: number;
  originals: number;
  datevExports: number;
  hasSigningKey: boolean;
  chainHead: string;
  createdAtMs: number;
  issues: string[];
}

export function backupStatus(): Promise<BackupStatus> {
  return invoke("backup_status");
}

export async function createBackup(destination: string, password: string): Promise<BackupResult> {
  const result = await invoke<BackupResult>("backup_create", { destination, password });
  await emitSettingsChanged("backup");
  return result;
}

export function previewRestore(source: string, password: string): Promise<RestorePreview> {
  return invoke("backup_preview", { source, password });
}

export async function confirmRestore(): Promise<RestorePreview> {
  const result = await invoke<RestorePreview>("backup_confirm");
  await emitSettingsChanged("backup");
  return result;
}

export async function resumeRestore(): Promise<RestorePreview> {
  const result = await invoke<RestorePreview>("backup_resume");
  await emitSettingsChanged("backup");
  return result;
}

export function formatBackupTime(ms?: number | null): string {
  if (!ms) return "noch nie";
  return new Date(ms).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" });
}
