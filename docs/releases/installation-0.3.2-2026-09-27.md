# Installation 0.3.2 – 27.09.2026

Historischer Stand. Aktuell ist [Installation 0.3.3](installation-0.3.3-2026-09-27.md): Anwendung 0.3.3 mit Drucker 0.1.0.12, Druckablauf bestanden.

Dieser Build einschließlich der Fehler- und UI-Korrekturen aus `bugfix-audit-2026-09-16.md` wurde im normalen Benutzerkonto installiert. Das enthaltene Druckerpaket war 0.1.0.6. **Die Installation war abgeschlossen; der echte Druckablauf blieb fehlerhaft.**

## Ein gemeinsamer Installer

`artifacts/windows/E-Rechnungs-Assistent-0.3.2-x64-Setup.exe` (464.397.345 Bytes) enthält die Anwendung 0.3.2, das Druckerpaket 0.1.0.6, dessen .NET-Laufzeit, Windows App Runtime 1.8, den WebView2-Offline-Installer sowie Java und die Rechnungsprüfer. Für die Installation müssen keine weiteren Installer manuell ausgeführt werden.

SHA-256: `6b927b1d18467d2cbed811dc5dd45584abcb6923a25b110aaa9b95f512787bca`.

Das Setup lief am 27.09.2026 von 11:47:01 bis 11:47:55 mit `/S /UPDATE`, ohne Rechteerhöhung, mit Exitcode 0. Sein extrahiertes Manifest enthält `requestedExecutionLevel="asInvoker"`. Installationsziel: `%LOCALAPPDATA%\E-Rechnungs-Assistent`.

## Nachweise

- Anwendung und HKCU-Deinstallationseintrag melden 0.3.2; der installierte Prozess startet und reagiert.
- Die EXE entspricht dem Release-Build einschließlich der erwarteten dreiby­tigen Tauri-NSIS-Kennzeichnung `UNK` → `NSS`.
- Alle 419 installierten Prüferdateien entsprechen ihren Build-Quellen.
- Das Druckerpaket 0.1.0.6 ist registriert und hat Status `Ok`; fünf zentrale installierte Dateien entsprechen dem gebündelten MSIX. Die Warteschlange `E-Rechnung` ist vorhanden.
- Windows App Runtime 1.8, Paketversion 8000.994.2142.0, ist installiert.
- Alle 56 vor dem Update erfassten Nutzerdatendateien waren direkt nach der Installation unverändert. Der Update-Snapshot unter `%LOCALAPPDATA%\de.erechnung.converter\update-backup\0.3.2` enthält Arbeitsdatenbank, Archivdatenbank, Korrekturwissen, Signaturschlüssel und Wiederherstellungshinweise. Die Versionsmarke lautet 0.3.2.
- Das Build-Gate bestand TypeScript/UI/Korpus, Rust-Clippy und Tests, .NET-Druckkern, XML- und PDF-Prüfungen sowie isolierte Update-Prüfungen. Die acht gezielten Installer-/Release-Gate-Tests bestanden ebenfalls.
- Die native Fensterprüfung im Build-Gate verweist auf einen älteren Nachweis. Sie ist kein neuer visueller UI-Test vom 27.09.2026; die native UI-Automation war in dieser Sitzung nicht erreichbar.

Maschinenlesbare lokale Nachweise: `artifacts/install-0.3.2-result.json`, `artifacts/installed-0.3.2-verification.json`, `artifacts/install-0.3.2-runtime.json`, `artifacts/release-gate.json`. Der Verifikationsbericht mit `passed: true` betrifft Dateiintegrität und Installation, nicht den Druckablauf.

## Behobenes Build-Problem

Der Build einschließlich NSIS-Paketierung war erfolgreich, aber die abschließende Signaturabfrage scheiterte an einem von npm/PowerShell vererbten Modulpfad. `scripts/build-windows-installer.ps1` lädt jetzt `Microsoft.PowerShell.Security` explizit aus dem Modulverzeichnis des ausführenden PowerShell-Prozesses. Die Signaturabfrage wurde damit geprüft; Kopie und Prüfsummenerzeugung für diesen Build wurden nachgeholt. Das Buildprotokoll bewahrt den ursprünglichen Abschlussfehler.

## Offene Grenzen

Der aktuelle GDI-Test scheitert bei `PrintDocument.Print()` mit Win32-Fehler 3003 (`ERROR_SPL_NO_STARTDOC`). Der unabhängige WPF/XPS-Test scheitert mit `PrintingCanceledException`. Die erneute Registrierung des installierten Pakets ohne Rechteerhöhung beseitigt den Fehler nicht. Es entsteht keine neue PDF-Übergabe. Derselbe Fehler war bereits am 11.09.2026 für 0.3.1 dokumentiert; eine Ursache ist weiterhin nicht belegt. Systemdienste wurden nicht neu gestartet und Windows wurde nicht neu gestartet.

Das äußere Setup ist unsigniert; das Druckerpaket trägt ein Entwicklungszertifikat, dem dieser Rechner bereits vertraut. Es wurden keine Zertifikatsspeicher oder Sicherheitsrichtlinien verändert. **Eine erstmalige vollständige Installation auf einem beliebigen fremden Rechner ohne Admin ist damit noch nicht erfüllt.** Dafür fehlt eine öffentlich vertrauenswürdige Paketsignatur beziehungsweise eine bereits durch die IT eingerichtete Vertrauensstellung. Siehe [Microsoft: MSIX-Signierung](https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide).

Der offizielle DATEV-Test, `cargo-audit` und Produktionssignaturen bleiben im Release-Gate offen. Dieser Build ist kein vollständig freigegebenes Produktionsrelease.
