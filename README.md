<p align="center">
  <img src="apps/desktop/app-icon.svg" alt="Druckwandel-Logo" width="96">
</p>

<h1 align="center">Druckwandel</h1>

<p align="center"><strong>E-Rechnungen aus dem Druckdialog.</strong></p>

<p align="center">
  <a href="https://github.com/Lauorez/druckwandel/actions/workflows/release-gate.yml"><img src="https://github.com/Lauorez/druckwandel/actions/workflows/release-gate.yml/badge.svg" alt="Release-Gate"></a>
  <a href="https://github.com/Lauorez/druckwandel/releases/latest"><img src="https://img.shields.io/github/v/release/Lauorez/druckwandel" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/Lizenz-MIT-blue.svg" alt="Lizenz: MIT"></a>
  <img src="https://img.shields.io/badge/Status-Beta-orange.svg" alt="Status: Beta">
  <a href="https://github.com/sponsors/Lauorez"><img src="https://img.shields.io/badge/Sponsor-GitHub%20Sponsors-ea4aaa?logo=githubsponsors&amp;logoColor=white" alt="Sponsor auf GitHub"></a>
</p>

**Behalte deinen bisherigen Rechnungsworkflow – Druckwandel macht daraus eine gültige E-Rechnung.**

Druckwandel (bis Version 0.3.4 „E-Rechnungs-Assistent“) ist eine lokale Desktop-Anwendung für Windows 11. Sie übernimmt Rechnungen aus Word, Excel, Branchen- und Altsoftware über einen virtuellen Drucker „E-Rechnung“, erkennt die Rechnungsdaten, lässt sie prüfen und ergänzen und erzeugt daraus **XRechnung** (UBL) oder **ZUGFeRD/Factur-X** (PDF/A-3 mit CII). Die Verarbeitung läuft vollständig lokal. Es werden keine Rechnungsdaten an externe Dienste gesendet.

> **English summary:** A local-first Windows desktop app that turns ordinary printed or PDF invoices into German/EU-compliant e-invoices (XRechnung, ZUGFeRD/Factur-X, EN 16931). It ships a driverless virtual printer, rule-based PDF extraction with a learning template memory, bundled offline validators (KoSIT, Mustang, veraPDF), a tamper-evident local archive, encrypted backups and a DATEV export. Built with TypeScript, React, Tauri/Rust and .NET. The UI and docs are in German.

> [!IMPORTANT]
> Das Projekt ist eine **Beta** (keine 1.0- und keine Kundenfreigabe). Eine technisch gültige E-Rechnung ist keine Garantie für die sachliche oder steuerliche Richtigkeit. Die Software ersetzt weder eine Steuerberatung noch eine revisionssichere Archivierung. Was bereits auf echten Systemen geprüft ist und was nicht, steht in der [Abnahmematrix](docs/acceptance-matrix.md).

## Funktionen

- **Virtueller Drucker „E-Rechnung“** für Windows 11 (Print Support Virtual Printer ohne eigenen Treiber, NativeAOT-Background-Task) mit lokaler OXPS-zu-PDF-Konvertierung
- **PDF-Erkennung**: Textlayer mit Bounding Boxes, Zeilen- und Tabellenrekonstruktion, regelbasierte Feldklassifikation mit Confidence und Quellenangabe, optionaler lokaler OCR-Fallback
- **Prüfoberfläche** mit Quellmarkierung im PDF, editierbaren Positionen, deutscher Betragseingabe und sichtbarer Pflichtfeldvalidierung
- **Vorlagengedächtnis** in auswählbaren Erkennungsprofilen: lernt bestätigte Korrekturen für gleich aufgebaute Folgerechnungen, ohne konkrete Rechnungswerte zu speichern
- **Rechnungsumfang**: Rechnung, Gutschrift, Rechnungskorrektur, Nachlässe/Zuschläge, Abschlags-, Anzahlungs- und Schlussrechnungen; Standardsteuer, Reverse Charge, steuerfrei, innergemeinschaftliche Lieferung, steuerbare 0 %
- **Ausgabeformate**: XRechnung 3.0 (UBL 2.1) und ZUGFeRD / Factur-X EN16931 (UN/CEFACT CII, eingebettet in eine neue PDF/A-3)
- **Verbindliche Offline-Prüfung** jeder Ausgabe mit gebündeltem KoSIT-Validator, Mustang und veraPDF
- **Exaktes Rechnen**: kanonisches, formatunabhängiges Rechnungsmodell, Dezimalarithmetik und deterministische kaufmännische Rundung
- **Posteingang** mit automatischer Entwurfssicherung und Wiederaufnahme nach Neustart
- **Rechnungsarchiv** mit SQLite-Suche, verketteten SHA-256-Prüfsummen und optionaler Ed25519-Bestätigung
- **Sicherung und Wiederherstellung** in eine kennwortgeschützte, mit `age` verschlüsselte Datei; datensparsamer Diagnosebericht
- **DATEV-Export** (EXTF-Buchungsstapel und Belegpaket, Entwicklungsstand)

## Installation

Voraussetzung: Windows 11 Version 24H2 oder neuer (Build 26100), 64-Bit. Die aktuelle Fassung ist **0.3.4** mit Drucker **0.1.0.12**.

Setup und Prüfsumme stehen im [Release v0.3.4](https://github.com/Lauorez/druckwandel/releases/tag/v0.3.4). Diese Fassung ist noch unter dem früheren Namen erschienen:

- [E-Rechnungs-Assistent-0.3.4-x64-Setup.exe](https://github.com/Lauorez/druckwandel/releases/download/v0.3.4/E-Rechnungs-Assistent-0.3.4-x64-Setup.exe)
- [SHA-256](https://github.com/Lauorez/druckwandel/releases/download/v0.3.4/E-Rechnungs-Assistent-0.3.4-x64-Setup.exe.sha256) (`723b4e0004899ad20e1eb66c6ebfb75bd16446e76b32ae2b82d75f50442cc661`)
- [Öffentliches Druckerzertifikat für die IT](https://github.com/Lauorez/druckwandel/releases/download/v0.3.4/ERechnung.Dev.cer)

Das Setup braucht Administratorrechte, weil es einen Drucker einrichtet. Version 0.3.4 installiert nach `%LOCALAPPDATA%\E-Rechnungs-Assistent` und legt den Drucker **E-Rechnung** an. Ab der nächsten Version installiert das Setup nach `C:\Program Files\Druckwandel` und ersetzt dabei eine vorhandene Installation des E-Rechnungs-Assistenten; Daten, Archiv und Drucker bleiben erhalten. Drucker und Daten gehören zu dem Konto, das die Einrichtung ausführt. Kommt die Administratorbestätigung von einem anderen Konto, bricht das Setup mit einem Hinweis ab. Node, Rust, .NET SDK oder Java sind dafür nicht nötig. Ein Update behält Entwürfe, Archiv und Vorlagengedächtnis. Die Deinstallation entfernt den Drucker, nicht `Dokumente\E-Rechnungsarchiv`.

> [!WARNING]
> **Signatur:** Das Setup ist unsigniert. Das Druckerpaket ist mit dem selbstsignierten Entwicklungszertifikat `CN=ERechnung Development` signiert. Das Setup prüft Zertifikat und Paket und hinterlegt nur das öffentliche Zertifikat in `LocalMachine\TrustedPeople`. Details: [Windows-Vorführpaket](docs/windows-demo-installation.md), [Installationsbericht 0.3.4](docs/releases/installation-0.3.4-2026-09-27.md).

Wie es nach der Installation weitergeht, beschreibt das [Benutzerhandbuch](docs/benutzerhandbuch.md).

## Aus dem Quellcode bauen (Windows)

Nach dem Klonen reicht ein Skript. Es installiert fehlende Werkzeuge (Node 22, Rust, .NET SDK 10) nur für das aktuelle Benutzerkonto, lädt das Prüfpaket samt Windows-JRE, erzeugt eine Musterrechnung und baut den NSIS-Installer:

```powershell
git clone https://github.com/Lauorez/druckwandel.git
cd druckwandel
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup-windows.ps1
```

Ergebnis: `artifacts\windows\Druckwandel-<Version>-x64-Setup.exe` mit SHA-256-Prüfsumme und `artifacts\demo\muster-rechnung.pdf`.

Die Setup-Datei enthält Anwendung, Drucker, Windows App Runtime, WebView2-Offline-Installer und die lokalen Prüfer samt Java. `validators:fetch` muss auf Windows laufen, weil die gebündelte JRE plattformabhängig ist; eine auf dem Mac geladene JRE darf nicht in den Windows-Installer.

```powershell
npm run installer:windows                          # gemeinsamer Installer (führt vorher das Release-Gate aus)
npm run installer:windows -- -SigningMode Production   # mit externem Produktionszertifikat
npm run printer:build                              # nur das Drucker-MSIX
npm run printer:install                            # nur das Druckerpaket neu setzen
npm run printer:uninstall
npm run printer:repair                             # Spooler-Reparatur (Administratorrechte)
```

`printer:install` setzt voraus, dass Windows der Paketsignatur bereits vertraut. Beim Entwicklungszertifikat ist dafür ein vorheriger Import nach `LocalMachine\TrustedPeople` nötig; der Benutzer-Zertifikatspeicher genügt nicht. Mehr zum Drucker: [printer/README.md](printer/README.md).

## Entwicklung

Fachkern, Tests und Oberfläche laufen auf Windows, macOS und Linux. Benötigt werden Node.js 22 sowie für die native App Rust (siehe `rust-toolchain.toml`) und die [Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/).

```sh
npm install
npm run desktop               # Tauri-App im Entwicklungsmodus
npm run desktop:web:dev       # nur die Oberfläche im Browser (ohne native Funktionen)
npm run extract:pdf -- rechnung.pdf [--ocr]
npm run generate:fixtures     # Beispiel-XML für XRechnung und ZUGFeRD
npm run demo:invoice          # Musterrechnung als PDF
```

Die E-Rechnungs-API liegt in `src/engine/index.ts`, die PDF-Pipeline in `src/extraction/index.ts`. Beträge und Mengen sind absichtlich Dezimal-Strings; JavaScript-`number` ist für Geldwerte nicht Teil des Domänenmodells.

### Projektstruktur

```text
src/                     TypeScript-Fachkern (plattformunabhängig)
  domain/                Rechnungsmodell, Berechnung, Validierung
  engine/                UBL/XRechnung, CII/ZUGFeRD, Hybrid-PDF, Prüfer-Adapter
  extraction/            PDF-Text, Layout, Tabellen, Feldklassifikation, OCR
  learning/              Vorlagengedächtnis und Erkennungsprofile
  policy/                Belegarten, Steuerfälle, nicht unterstützte Fälle
  export/datev/          DATEV-EXTF und Belegpaket
apps/desktop/            Tauri-Desktop-App
  src/                   React-Oberfläche
  src-tauri/             Rust-Backend: Dateien, SQLite, Archiv, Sicherung, Prüferstart, Deep Links
printer/                 Virtueller Windows-Drucker (.NET, WinUI 3, MSIX)
scripts/                 Build-, Installer-, Release-Gate-, Korpus- und Smoke-Test-Skripte
test/                    Vitest-Suite, Fixtures und Referenzkorpus
docs/                    Dokumentation
```

Datenfluss und Sicherheitsgrenzen: [docs/architecture.md](docs/architecture.md).

### Tests und Prüfgates

```sh
npm run check          # Core- und Desktop-Typprüfung, Tests inkl. UI, Referenzkorpus, Fuzzing
npm run check:native   # Rust: Clippy mit -D warnings und cargo test
npm run check:xml      # KoSIT/Mustang-Regressionen aller unterstützten Fälle als UBL und CII
npm run check:pdf      # Musterrechnung als Hybrid-PDF, geprüft mit veraPDF und Mustang
npm run release:gate   # alles zusammen inkl. .NET, Schwachstellen, Installer-Szenarien, Abnahmematrix
```

`check:xml` und `check:pdf` brauchen die gebündelte JRE (`npm run validators:fetch`). Das Korpus-Gate (`npm run corpus:check`) erzeugt ein anonymisiertes Referenzkorpus, prüft Felder, Positionen und blockierte Sonderfälle und testet 250 reproduzierbare synthetische Rechnungen, siehe [docs/corpus.md](docs/corpus.md).

Die CI führt `npm run release:gate -- --portable` auf Ubuntu aus. Schritte, die Windows oder die gebündelten Prüfer brauchen, gelten dort als „pending“, nicht als bestanden.

## Externe Validierung

In der installierten App entsteht eine fertige Ausgabe erst nach unabhängiger Prüfung der tatsächlich erzeugten Bytes: KoSIT für XRechnung, Mustang für ZUGFeRD/Factur-X-XML und veraPDF (PDF/A-3b) samt bytegleichem XML-Extrakt für PDF-Rechnungen. Versionen und Lizenzen des Pakets: [`apps/desktop/src-tauri/resources/validators/`](apps/desktop/src-tauri/resources/validators/README.md).

Für die Entwicklung lassen sich eigene Prüfer-Installationen direkt aufrufen:

```powershell
$env:KOSIT_VALIDATOR_JAR = 'C:\validator\validator.jar'
$env:KOSIT_SCENARIOS = 'C:\xrechnung-config\scenarios.xml'
npm run validate:external -- test/fixtures/generated/xrechnung-ubl.xml

$env:MUSTANG_VALIDATOR_JAR = 'C:\validator\Mustang-CLI.jar'
npm run validate:zugferd -- test/fixtures/generated/zugferd-en16931.xml
```

KoSIT und Mustang prüfen unterschiedliche Profile. Eine Factur-X-CII darf deshalb nicht als XRechnung-Ergebnis des KoSIT-Szenarios bewertet werden.

## Unterstützte Standards und Grenzen

| Standard | Stand |
| --- | --- |
| XRechnung | 3.0.2, KoSIT-Konfiguration 2026-01-31, Prüfmotor 1.6.3 |
| ZUGFeRD / Factur-X | EN16931, Profilkennung `urn:cen.eu:en16931:2017`, geprüft mit Mustang 2.26.0 |
| PDF | PDF/A-3b, geprüft mit veraPDF 1.28.2 |
| Syntax | UBL 2.1, UN/CEFACT CII D16B |

**Noch nicht unterstützt** sind Rundungsausgleich, mehrere Zahlungswege, weitere Steuerkategorien, Export/Ausfuhr, Istversteuerung und Peppol. Der DATEV-Export bleibt für Gutschriften/Korrekturen, Abschlags-/Anzahlungs-/Schlussrechnungen und die Steuerfälle AE/E/K/Z gesperrt. Scans brauchen den lokalen OCR-Weg oder manuelle Erfassung. Innergemeinschaftliche Lieferungen benötigen ein Leistungsdatum und explizite Lieferangaben, bei XRechnung auch Lieferort und Postleitzahl.

Die automatischen Prüfungen ersetzen keinen echten DATEV-Testimport und keinen Installations- und Drucktest auf dem Zielsystem. Die nächsten Schritte bis 1.0 stehen in der [Roadmap](docs/planning/roadmap-1.0.md).

## Dokumentation

Eine Übersicht aller Dokumente steht in [docs/README.md](docs/README.md), darunter [Benutzerhandbuch](docs/benutzerhandbuch.md), [Architektur](docs/architecture.md), [DATEV-Export](docs/datev-export.md), [gebündelte Komponenten](docs/components.md) sowie Abnahme-, Prüf- und Installationsberichte.

## Mitwirken

Beiträge sind willkommen! Bitte lies vorher [CONTRIBUTING.md](CONTRIBUTING.md) und den [Verhaltenskodex](CODE_OF_CONDUCT.md). Sicherheitslücken bitte privat melden, siehe [SECURITY.md](SECURITY.md). Und bitte niemals echte Rechnungen in Issues oder Pull Requests hochladen.

## Lizenz

Der Quellcode steht unter der [MIT-Lizenz](LICENSE). Die gebündelten Prüfwerkzeuge und einige Assets haben eigene Lizenzen, siehe [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
