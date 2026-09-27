import { invoke } from "@tauri-apps/api/core";

export interface DiagnosticComponent {
  name: string;
  version: string;
  status: string;
}

export interface DiagnosticReport {
  schemaVersion: number;
  createdAtMs: number;
  appVersion: string;
  os: string;
  arch: string;
  components: DiagnosticComponent[];
  archiveEntries: number;
  signedEntries: number;
  signingEnabled: boolean;
  workspaceDocuments: number;
  datevProfilePresent: boolean;
  backupReminderDue: boolean;
  pendingRestore: boolean;
  notes: string[];
}

export function loadDiagnosticReport(): Promise<DiagnosticReport> {
  return invoke("diagnostic_report");
}

export function saveDiagnosticReport(destination: string): Promise<string> {
  return invoke("write_diagnostic_report", { destination });
}

export function diagnosticPreview(report: DiagnosticReport): string {
  return JSON.stringify(report, null, 2);
}
