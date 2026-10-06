# Windows-Druckbrücke für Druckwandel

Lokaler virtueller PDF-Drucker für Windows 11. Der installierte Drucker **E-Rechnung** übernimmt einen normalen Windows-Druckauftrag, erzeugt ohne Speichern-unter-Dialog ein PDF und übergibt ihn über die kleine native Windows-Druckbrücke an die lokale Anwendung Druckwandel.

> Paketversion: **0.1.0.4**. Build, MSIX-Inhalt, Installation, Druckerregistrierung sowie OXPS-Druck und PDF-Konvertierung sind auf Windows 11 praktisch bestätigt.

Diese Beta ist für einen ersten End-to-End-Test gedacht. Sie ist noch keine produktiv signierte oder allgemein verteilbare Anwendung.

## Scope

Enthalten:

- moderner Print Support Virtual Printer ohne eigenen V3-/V4-Treiber,
- OXPS-zu-PDF über Windows `PrintWorkflowPdlConverter`,
- PDF-Passthrough für kompatible Anwendungen,
- UUID-basierte PDF- und JSON-Ablage,
- atomarer Job-Store und lokales JSONL-Logging,
- schlanke WinUI-3-Druckbrücke mit atomarer Übergabe,
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

## Entwicklungspaket installieren

Im Wurzelverzeichnis des Gesamtprojekts wird der bestätigte Build- und Installationsweg verwendet:

```powershell
npm run printer:build
# anschließend in einer PowerShell als Administrator:
npm run printer:install
```

Das Zertifikat ist ausschließlich für lokale Entwicklungstests bestimmt. Für verteilbare Pakete ist ein vertrauenswürdiges Codesigning-Zertifikat oder Store-Signing erforderlich.

## Schnellstart auf Windows

```powershell
git clone https://github.com/Lauorez/druckwandel.git
cd druckwandel\printer
# PowerShell zuvor als Administrator öffnen
Set-ExecutionPolicy -Scope Process Bypass
./scripts/build.ps1
./scripts/install.ps1
```

Danach prüfen:

```powershell
Get-Printer -Name "E-Rechnung"
```

Anschließend Notepad öffnen und über **Drucken → E-Rechnung** drucken. Windows darf keinen zusätzlichen Speichern-unter-Dialog anzeigen. Nach der Konvertierung übergibt die Druckbrücke den Auftrag an `erechnung-review://print-job/<UUID>` und schließt sich; bei installiertem Protokollhandler öffnet sich Druckwandel.

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

Das Development-Zertifikat wird unter `LocalMachine\TrustedPeople` importiert; deshalb muss PowerShell für die Installation als Administrator laufen. Anschließend werden das neueste erzeugte MSIX und dessen architekturpassende Windows-App-Runtime-Abhängigkeit installiert.

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

Vor der Review-Übergabe liegen die Quelldateien in diesem geschützten Speicherort. Die Companion kopiert das fertige PDF anschließend zusammen mit `<UUID>.printjob.json` atomar nach `Dokumente\E-Rechnung Druckeingang`.

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

Die erweiterte Desktop-Matrix prüft zusätzlich drei Seiten, A4-Querformat, Grafik/Farbe/Tabelle und zwei parallele Jobs:

```powershell
./scripts/smoke-install.ps1 -TestMatrix
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
          → Companion-App → Review-Inbox + erechnung-review://print-job/<UUID>
```

Das Manifest enthält absichtlich kein `OutputFileTypes`. Dadurch wird die Queue nicht als klassischer File Printer registriert und Windows sollte keinen Speichern-unter-Dialog anzeigen.

Details: [docs/architecture.md](docs/architecture.md) und [docs/windows-print-api-notes.md](docs/windows-print-api-notes.md).
## Bekannte Einschränkungen

- Der OXPS-Druckpfad, Mehrseitigkeit, A4-Querformat, Grafik/Farbe/Tabelle und zwei parallele Jobs sind auf einem interaktiven Windows-11-x64-System bestätigt; Browser, Word, Excel und ARM64 stehen noch aus.
- Die aktuelle C#-WinRT-Projektion der `PrintSupportJobUI`-Aktivierungsargumente muss praktisch bestätigt werden.
- Die PDC-Datei bietet zunächst A3, A4, A5, Hoch-/Querformat, Farbe und 600 dpi; weitere PrintTicket-Optionen fehlen.
- Bei erfolgreicher Review-Übergabe schließt die Companion den Print-Workflow automatisch. Schlägt die Protokollaktivierung fehl, bleibt sie mit einer sichtbaren Fehlermeldung und einer Retry-Schaltfläche geöffnet.
- Die Development-Signatur ist nicht für Distribution geeignet.
- Die mitgelieferten App-Icons sind Platzhalter aus dem Microsoft-Sample.

Änderungen dieser und späterer Versionen stehen in [CHANGELOG.md](CHANGELOG.md).

## Sicherheits- und Datenschutzmodell

- keine Netzwerkaufrufe im Produktcode,
- keine Telemetrie oder Analytics,
- keine externen PDF-Konverter,
- keine vorhersehbaren Dateinamen aus Dokumenttiteln,
- Übergabedaten ausschließlich im lokalen Paketverzeichnis und im lokalen Review-Eingang unter `Dokumente\E-Rechnung Druckeingang`.
