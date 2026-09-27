# Installation 0.3.3 – 27.09.2026

Anwendung **0.3.3** und Drucker **0.1.0.12** sind im normalen Benutzerkonto installiert. Der echte Druckablauf besteht. Der Bericht zu [0.3.2](installation-0.3.2-2026-09-27.md) ist historisch: dort war das defekte Druckerpaket 0.1.0.6 enthalten, und der Druck endete vor der PDF-Übergabe.

## Ursache und Korrektur

`StartDocPrinter` scheiterte in 0.1.0.6 mit `0xFFFF`, weil der NativeAOT-Task `TriggerDetails` nur als `WinRT.IInspectable` sah und den Auftrag beendete. Ein .NET-Host druckte, lud die Laufzeit aber aus `C:\Program Files\dotnet`. Die bereinigte Fassung behält die explizite Projektion `MarshalInspectable<PrintWorkflowVirtualPrinterTriggerDetails>.FromAbi` und gibt den ABI-Zeiger im `finally` frei. Diagnostik, Dateilog und der `WinRT.Host`-Build sind entfernt. Das Paket enthält `ERechnung.VirtualPrinter.Native.dll` und die WINMD; der Aktivierungstest lädt kein CoreCLR.

## Installer

`artifacts/windows/E-Rechnungs-Assistent-0.3.3-x64-Setup.exe` (464.679.714 Bytes).

SHA-256: `c82d547e919d02768f7c636fb9f117c8f36cba10e1cbb088b931b18a15fcb89b`.

Das Setup lief am 27.09.2026 von 15:46:21 bis 15:47:05 mit `/S /UPDATE`, ohne Rechteerhöhung, mit Exitcode 0. Installationsziel: `%LOCALAPPDATA%\E-Rechnungs-Assistent`. Es enthält Anwendung 0.3.3, Drucker-MSIX 0.1.0.12, die mitgelieferte Laufzeit der Druckoberfläche, Windows App Runtime 1.8, den WebView2-Offline-Installer sowie Java und die Rechnungsprüfer.

Der abschließende `Get-FileHash`-Aufruf im Build brauchte zusätzlich das Modul `Microsoft.PowerShell.Utility` aus dem Windows-PowerShell-Verzeichnis. `scripts/build-windows-installer.ps1` lädt es jetzt zusammen mit `Microsoft.PowerShell.Security`. Die Prüfsumme dieser Datei wurde damit geschrieben.

## Nachweise

- Anwendung und HKCU-Deinstallationseintrag melden 0.3.3. Die installierte EXE entspricht dem Release-Build einschließlich der Tauri-Kennzeichnung `UNK` → `NSS`, Dateiversion 0.3.3. Das Fenster „E-Rechnungs-Assistent“ antwortet.
- Alle 419 installierten Prüferdateien entsprechen ihren Build-Quellen.
- Fünf zentrale Druckerdateien, darunter `ERechnung.VirtualPrinter.Native.dll`, entsprechen dem im Setup gebündelten MSIX. Die Warteschlange `E-Rechnung` verwendet den Virtual Print Class Driver. Windows App Runtime 1.8, Paketversion 8000.994.2142.0, ist installiert.
- Alle 74 vor dem Update erfassten Nutzerdatendateien waren nach der Installation unverändert. Der Update-Snapshot liegt unter `%LOCALAPPDATA%\de.erechnung.converter\update-backup\0.3.3`.
- Das Release-Gate bestand TypeScript/UI/Korpus, Rust-Clippy und Tests, .NET-Druckkern, XML- und PDF-Prüfungen sowie die isolierte Update-Prüfung. DATEV-Prüfprogramm, `cargo-audit` und Produktionssignaturen bleiben offen.
- Direkter `StartDocPrinter`-Test gegen das installierte Paket: RAW, XPS_PASS und Standardformat, Job-IDs 16–18.
- WPF/XPS: eine Seite, Jobstatus `jobCompleted`, Spooler-Job 19.
- GDI-Kaltstart `scripts/smoke-print-workspace.ps1`: PASS. Entwurf `WP7 Drucktest 0e61869c492c49279b534c067a6ad025`, Original-SHA-256 `1197a57fae3f8aa0853a98ae14fb0b2468b9d5c6a14cf365b22285ddfa1d55ca`.

Vor dem Setup war der Drucker auf diesem Rechner bereits als 0.1.0.12 registriert. Das Setup hat ihn deshalb nicht ersetzt (`Der E-Rechnungsdrucker ist bereits eingerichtet.`). Die beiden Builds derselben Version unterschieden sich in der nativen DLL. Für den Dateivergleich und den abschließenden Drucktest wurde das genaue MSIX aus dem Setup-Payload ohne Rechteerhöhung erneut registriert. Ein Update von einer älteren Druckerversion übernimmt das neuere Paket direkt.

Maschinenlesbare Nachweise: `artifacts/install-0.3.3-result.json`, `artifacts/installed-0.3.3-verification.json`, `artifacts/install-0.3.3-runtime.json`, `artifacts/wp7-print-smoke.json`.

## Offene Grenze

Das äußere Setup ist unsigniert. Das Drucker-MSIX ist mit dem Entwicklungszertifikat signiert, dem dieser Rechner bereits vertraut. Es wurden keine Zertifikatsspeicher oder Sicherheitsrichtlinien verändert. Eine öffentlich vertrauenswürdige Codesignatur liegt nicht vor. Eine Erstinstallation auf einem fremden Rechner ohne vorheriges Zertifikatvertrauen braucht diese Signatur. Die Druckkorrektur hängt davon nicht ab. Siehe [Microsoft: MSIX-Signierung](https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide).
