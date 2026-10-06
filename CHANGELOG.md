# Changelog

Alle nennenswerten Änderungen an Druckwandel (bis 0.3.4 „E-Rechnungs-Assistent“) werden in dieser Datei dokumentiert. Das Format orientiert sich an [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), die Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

Änderungen am virtuellen Drucker stehen zusätzlich in [drucker/CHANGELOG.md](drucker/CHANGELOG.md). Ausführliche Prüfberichte je Version liegen unter [docs/releases/](docs/releases/).

## [Unveröffentlicht]

### Hinzugefügt

- Veröffentlichung als Open-Source-Projekt unter der MIT-Lizenz mit Beitragsleitfaden, Verhaltenskodex, Sicherheitsrichtlinie, Drittanbieterhinweisen sowie Issue- und Pull-Request-Vorlagen.
- Benutzerhandbuch und Dokumentationsübersicht unter `docs/`.
- Die Drucker-Workflows liegen jetzt unter `.github/workflows/` im Repository-Root und laufen damit auf GitHub. Dependabot deckt npm, Cargo, NuGet und GitHub Actions ab.

### Geändert

- Die Anwendung heißt jetzt **Druckwandel**, das Repository `Lauorez/druckwandel`. Das Setup installiert nach `%LOCALAPPDATA%\Druckwandel` und ersetzt eine vorhandene Installation des E-Rechnungs-Assistenten wie ein Update; Daten, Archiv, Vorlagengedächtnis und Drucker bleiben erhalten. Interne Kennungen (`de.erechnung.converter`, `erechnung-review://`, Drucker „E-Rechnung“, Ordner unter „Dokumente“, DATEV-Herkunftskennung) sind unverändert.
- npm-Skripte umbenannt: `wp5:*` → `printer:*`, `wp6:check` → `corpus:check`.
- Dokumentation unter `docs/` neu gegliedert (Abnahmeprotokolle, Entscheidungen, Prüfberichte, Installationsberichte, Planung).

### Behoben

- Lizenzangabe von KoSIT-Validator und XRechnung-Konfiguration im Validatorenmanifest und in `docs/components.md` korrigiert (Apache-2.0 statt EUPL-1.2).

## [0.3.4] – 2026-09-27

### Geändert

- Fehlt das Entwicklungszertifikat des Druckers in `LocalMachine\TrustedPeople`, fragt das Setup einmalig nach Administratorrechten. Ein erhöhter Hilfsschritt prüft Paket und Zertifikat erneut und importiert nur dieses Zertifikat. Anwendung und Drucker werden weiterhin im Benutzerkonto installiert.

## [0.3.3] – 2026-09-27

Enthält auch den nicht separat veröffentlichten Stand 0.3.2.

### Hinzugefügt

- Belegarten Gutschrift (381) und Rechnungskorrektur (384), positions- und belegweite Nachlässe und Zuschläge, Abschlags-, Anzahlungs- und Schlussrechnungen.
- Wählbare Steuerfälle: Reverse Charge, steuerfrei, innergemeinschaftliche Lieferung und steuerbare 0 %.
- Einstellungsfenster mit kennwortgeschützter Sicherung und Wiederherstellung (`.erechnung`, verschlüsselt mit `age`), Diagnosebericht ohne Rechnungsinhalte und Verwaltung der Erkennungsprofile.
- Unter Windows wird der lokale Archivschlüssel mit DPAPI geschützt.
- Messbare Erkennungsqualität (`artifacts/quality-report.json`) sowie anzeigbare, deaktivierbare und rückgängig machbare gelernte Zuordnungen.
- Release-Gate (`npm run release:gate`), Komponenteninventar und Abnahmematrix. Vor einem Update sichert das Setup die Nutzerdaten und weist ältere Versionen ab.

### Behoben

- Der Drucker-Background-Task ist mit NativeAOT gebaut und schließt echte Druckaufträge ab (Drucker 0.1.0.12).
- Zahlreiche Korrekturen an Beträgen, Validierung, UBL/CII-Serialisierung, DATEV-Verteilung, Speichern und Wiederherstellung, siehe [docs/audits/](docs/audits/).

## [0.3.1] – 2026-09-08

### Hinzugefügt

- Gemerkte PDF-Stellen liegen in auswählbaren Erkennungsprofilen.
- Fälligkeitsangaben wie „fällig in 14 Tagen“ und ausgeschriebene Datumsformate werden erkannt.
- Beim Zuordnen über **Im PDF markieren** wird der Wert gegen die Feldregeln geprüft.

### Geändert

- Robustere Adresserkennung. Gemerkte Stellen unterhalb der Positionstabelle folgen der Tabelle, wenn die Rechnung durch mehr Positionen länger wird.

## 0.3.0 – 2026-09-07 (ohne eigenes Release)

### Hinzugefügt

- Vollständige Tauri-Desktopanwendung mit Posteingang, automatischer Entwurfssicherung und Wiederaufnahme nach Neustart.
- Verbindliche lokale Prüfung mit gebündeltem KoSIT-Validator, Mustang und veraPDF vor jeder fertigen Ausgabe.
- Hybrid-PDF: neue PDF/A-3-Datei mit eingebetteter `factur-x.xml`.
- Rechnungsarchiv mit SQLite-Suche, verketteten SHA-256-Prüfsummen und optionaler Ed25519-Bestätigung.
- Entwicklungsstand des DATEV-Exports (EXTF-Buchungsstapel und Belegpaket).
- Gemeinsamer Windows-Installer mit eingebettetem virtuellen Drucker und das Setup-Skript `scripts/setup-windows.ps1`.

## [0.1.0-beta.1] – 2026-08-23

Erste Beta des virtuellen Windows-PDF-Druckers, siehe [drucker/CHANGELOG.md](drucker/CHANGELOG.md).

[Unveröffentlicht]: https://github.com/Lauorez/druckwandel/compare/v0.3.4...HEAD
[0.3.4]: https://github.com/Lauorez/druckwandel/releases/tag/v0.3.4
[0.3.3]: https://github.com/Lauorez/druckwandel/releases/tag/v0.3.3
[0.3.1]: https://github.com/Lauorez/druckwandel/releases/tag/v0.3.1
[0.1.0-beta.1]: https://github.com/Lauorez/druckwandel/releases/tag/v0.1.0-beta.1
