# E-Rechnung Virtual PDF Printer – Beta 1

Lokaler virtueller PDF-Drucker für Windows 11. Der installierte Drucker **E-Rechnung** übernimmt einen normalen Windows-Printjob, erzeugt ohne Speichern-unter-Dialog ein PDF und öffnet eine minimale Companion-App mit Job-Metadaten.

> Version: **0.1.0-beta.1**. Erste private Testversion. Build, MSIX-Inhalt, Installation, automatische Queue-Registrierung und Deinstallation sind auf einem frischen Windows-Build-26100-CI-System verifiziert. Der interaktive Print- und Companion-Pfad muss noch auf einem Windows-11-Desktop bestätigt werden.

Diese Beta ist für einen ersten End-to-End-Test gedacht. Sie ist noch keine produktiv signierte oder allgemein verteilbare Anwendung.

## Scope

Enthalten:

- moderner Print Support Virtual Printer ohne eigenen V3-/V4-Treiber,
- OXPS-zu-PDF über Windows `PrintWorkflowPdlConverter`,
- PDF-Passthrough für kompatible Anwendungen,
- UUID-basierte PDF- und JSON-Ablage,
- atomarer Job-Store und lokales JSONL-Logging,
- native WinUI-3-Companion-App,
- MSIX-Manifest, Entwicklungszertifikat und Installationsskripte.

Nicht enthalten sind PDF-Analyse, OCR, Rechnungsfelder, EN 16931, ZUGFeRD, XRechnung, PDF/A, Datenbank, Cloud oder produktive UI.

## Voraussetzungen

- Windows 11 24H2 oder neuer, mindestens Build `26100`
- x64; ARM64 ist vorbereitet, aber noch nicht getestet
- Visual Studio 2026 mit:
  - .NET Desktop Development
  - Windows application development / WinUI
  - MSIX Packaging Tools
  - Windows 11 SDK `10.0.26100` oder neuer
- .NET SDK 10
- PowerShell 5.1 oder 7
- Developer Mode ist für lokale MSIX-Tests empfohlen

Das Projekt verwendet die aktuelle Virtual-Printer-API, die erst mit Build 26100 eingeführt wurde. Ältere Windows-11-Versionen werden bewusst nicht unterstützt.

## Beta-Paket installieren

Die private GitHub-Prerelease `v0.1.0-beta.1` enthält ein x64-MSIX und das zugehörige öffentliche Entwicklungszertifikat. Beide Dateien in denselben Ordner herunterladen, PowerShell als Administrator öffnen und ausführen:

```powershell
Import-Certificate `
  -FilePath .\ERechnung.Dev.cer `
  -CertStoreLocation Cert:\LocalMachine\TrustedPeople
Add-AppxPackage .\CompanionApp_0.1.0.1_x64.msix
```

Das Zertifikat ist ausschließlich für diesen privaten Betatest bestimmt. Alternativ kann die Beta wie unten beschrieben aus dem Quellcode gebaut werden.

## Schnellstart auf Windows

```powershell
git clone https://github.com/Lauorez/erechnung.git
cd erechnung
# PowerShell zuvor als Administrator öffnen
Set-ExecutionPolicy -Scope Process Bypass
./scripts/build.ps1
./scripts/install.ps1
```

Danach prüfen:

```powershell
Get-Printer -Name "E-Rechnung"
```

Anschließend Notepad öffnen und über **Drucken → E-Rechnung** drucken. Windows darf keinen zusätzlichen Speichern-unter-Dialog anzeigen. Nach der Konvertierung öffnet sich die Companion-App.

## Build

Standardmäßig wird ein signiertes x64-Release-Paket erzeugt:

```powershell
./scripts/build.ps1
```

Weitere Varianten:

```powershell
./scripts/build.ps1 -Configuration Debug
./scripts/build.ps1 -Platform ARM64
```

Das Skript:

1. prüft Windows-Build, .NET und MSBuild,
2. erzeugt bei Bedarf ein lokales Code-Signing-Zertifikat unter `.cert/`,
3. restauriert die Pakete,
4. baut Background-Task, WinUI-App und MSIX,
5. legt die Pakete unter `artifacts/packages/` ab.

`dotnet build` allein ist für den vollständigen MSIX-Build nicht der unterstützte Pfad. `scripts/build.ps1` verwendet das MSBuild aus Visual Studio.

## Installation

```powershell
./scripts/install.ps1
```

Das Development-Zertifikat wird unter `LocalMachine\TrustedPeople` importiert; deshalb muss PowerShell für die Installation als Administrator laufen. Anschließend wird das neueste erzeugte MSIX installiert.

## Lokale Dateien

Die Dateien liegen im geschützten Local-State-Verzeichnis des Pakets:

```text
%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\ERechnung\
├─ PrintJobs\
│  ├─ <UUID>.pdf
│  └─ <UUID>.json
├─ Sessions\
│  └─ <SHA256(SessionId)>.txt
└─ Logs\
   └─ <UUID>.jsonl
```

Die Companion-App öffnet diesen Speicherort über **Open Folder**.

## Tests

Plattformunabhängige Unit-Tests:

```powershell
./scripts/test.ps1
```

Der vollständige manuelle Windows-Test steht in [docs/testing.md](docs/testing.md). Er umfasst Notepad, Browser, Word/Excel, Hoch-/Querformat, mehrere Seiten und parallele Jobs.

Für eine automatische Installation mit anschließender Deinstallation steht zusätzlich ein destruktiver Smoke-Test für CI- oder Wegwerf-Testsysteme bereit:

```powershell
./scripts/smoke-install.ps1
```

Das Skript installiert das Paket, wartet auf die Queue `E-Rechnung` und entfernt danach Paket, Queue und temporär vertrautes Zertifikat wieder.

Auf einer **interaktiven** Windows-Desktop-Sitzung kann derselbe Test zusätzlich eine A4-XPS-Testseite an die Queue senden und PDF, Metadaten sowie den erfolgreichen Konvertierungsstatus prüfen:

```powershell
./scripts/smoke-install.ps1 -TestPrint
```

Dabei werden die Jobdateien vor der Deinstallation zusätzlich unter `artifacts/smoke/` gesichert. Der verwendete gehostete GitHub-Windows-Runner hat den Druckjob in seiner nicht interaktiven Sitzung vor Aktivierung der Print-Workflow-Background-Task abgebrochen; CI prüft deshalb bewusst den installierbaren Paket- und Queue-Lifecycle, nicht die UI-Aktivierung.

Bei einem Fehler sammelt folgendes Skript Paket-, Queue-, Spooler- und relevante Windows-Ereignisdaten, ohne PDFs zu kopieren:

```powershell
./scripts/collect-diagnostics.ps1
```

Mit `-IncludeJobMetadata` werden zusätzlich JSON-Metadaten und JSONL-Statuslogs aufgenommen. Diese können Dokumentnamen enthalten und sollten vor dem Teilen geprüft werden.

## Deinstallation

```powershell
./scripts/uninstall.ps1
```

Mit dem MSIX-Paket sollte Windows auch die zugehörige Queue entfernen. Kontrolle:

```powershell
Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue
```

## Architektur

```text
Anwendung → Windows Print Pipeline → OXPS/PDF
          → VirtualPrinterBackgroundTask
          → XPS-to-PDF / PDF copy
          → lokaler atomarer Job-Store
          → PrintWorkflowUILauncher
          → Companion-App
```

Das Manifest enthält absichtlich kein `OutputFileTypes`. Dadurch wird die Queue nicht als klassischer File Printer registriert und Windows sollte keinen Speichern-unter-Dialog anzeigen.

Details: [docs/architecture.md](docs/architecture.md) und [docs/windows-print-api-notes.md](docs/windows-print-api-notes.md).

Der vollständige aktuelle Arbeitsstand für die Fortsetzung auf einem anderen Gerät steht in [docs/PROGRESS.md](docs/PROGRESS.md).

## Bekannte Einschränkungen

- Installation, Queue-Registrierung und Deinstallation sind automatisiert auf Windows Build 26100 verifiziert; der Druckpfad ist noch nicht in einer interaktiven Windows-11-Sitzung ausgeführt worden.
- Die aktuelle C#-WinRT-Projektion der `PrintSupportJobUI`-Aktivierungsargumente muss praktisch bestätigt werden.
- Die PDC-Datei bietet zunächst A3, A4, A5, Hoch-/Querformat, Farbe und 600 dpi; weitere PrintTicket-Optionen fehlen.
- Das Schließen der Companion-App beendet den Print-Workflow. Bleibt sie offen, bleibt auch die zugehörige UI-Aktivierung aktiv.
- Die Development-Signatur ist nicht für Distribution geeignet.
- Die mitgelieferten App-Icons sind Platzhalter aus dem Microsoft-Sample.

Änderungen dieser und späterer Versionen stehen in [CHANGELOG.md](CHANGELOG.md).

## Sicherheits- und Datenschutzmodell

- keine Netzwerkaufrufe im Produktcode,
- keine Telemetrie oder Analytics,
- keine externen PDF-Konverter,
- keine vorhersehbaren Dateinamen aus Dokumenttiteln,
- keine Rechnungsdaten außerhalb des lokalen Paketverzeichnisses.
