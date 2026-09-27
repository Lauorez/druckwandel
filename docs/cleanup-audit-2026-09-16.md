# Generalcheck – 16.09.2026

Geprüfter Arbeitsstand: E-Rechnungs-Assistent 0.3.1 unter Windows, einschließlich der uncommitteten Änderungen seit dem Cleanup vom 11.09. Schwerpunkt: Sicherung, Archivschutz/DPAPI, Diagnose, Einstellungsfenster, Update-Guard und Prozesssperre. Es wurde kein Commit erstellt und kein Installer über den echten Nutzerbestand ausgeführt.

## Behobene Fehler

| Bereich | Fehler und Korrektur |
| --- | --- |
| Sicherung | Eine geprüfte, noch nicht übernommene Wiederherstellung (`preview`) galt als unterbrochener Vorgang. Die Oberfläche zeigte „fortsetzen“, obwohl nur `replacing` fortsetzbar ist; nach einem Neustart fehlte die Bestätigung. `pendingRestore` gilt nur noch für `replacing`; eine geprüfte Sicherung bleibt über `checkedPreview` bestätigbar. |
| Sicherung | Nach erfolgreicher Übernahme blieb das entschlüsselte Staging inklusive des ungeschützten Archivschlüssels liegen. Staging wird nach Abschluss gelöscht; eine neue Prüfung räumt ein altes Preview-Staging ab. |
| Sicherung | DATEV unter `Steuerkanzlei` wurde bei der Übernahme nicht gesondert zur Seite gelegt, falls der Archivordner schon verschoben war. Der Ordner wird jetzt wie die übrigen Bestände nach `replaced/` verschoben, bevor die Sicherung einspielt. |
| Diagnose | `write_diagnostic_report` akzeptierte vom Client geliefertes JSON und prüfte nur eine schwache Verbotsliste. Es wird ausschließlich der serverseitig erzeugte Zähler-/Versionsbericht geschrieben. |
| Exklusivität | Mutierende Kommandos ohne `guard::exclusive()`: Signaturumschaltung, DATEV-Profil, DATEV-Ordneröffnung (`finish()`), Workspace-Aktivierung/Fehler/Scan/Verwerfen und Legacy-Entwurf. Alle nehmen jetzt die Prozesssperre. |
| DATEV | `finish` und `datev_open_export` prüfen die Exportkennung als UUID, bevor Dateien geschrieben oder Explorer geöffnet wird. |
| Einstellungen | Escape/Schließen wartete nur auf DATEV-Schreibvorgänge. Erkennungsprofil-Änderungen werden ebenfalls vor dem Verlassen abgeschlossen. |
| Erinnerung | Die Sicherungserinnerung im Hauptfenster blieb nach einer erfolgreichen Sicherung in derselben Sitzung stehen. Sie folgt jetzt dem Ereignis `app-settings-changed` mit Umfang `backup`. |
| Installer | `UpdateGuard` schrieb `installed-version.txt` bereits in `PrepareUpdate`, also vor Drucker- und Dateiinstallation. Die Marke wird erst in `NSIS_HOOK_POSTINSTALL` über `RecordInstalledVersion` gesetzt. |
| WP14-Smoke | Der Klickhelfer wertete `element.click()` als Fehlschlag (`undefined !== undefined`) und erkannte das Einstellungsfenster nicht, wenn Dokumenttitel und URL ohne `#settings` blieben. Der Lauf besteht isoliert. |

## Verifikation

| Prüfung | Ergebnis |
| --- | --- |
| `git diff --check` | Keine Whitespace-Fehler |
| `npm run check` | Core-/Desktop-/UI-Test-Typprüfung erfolgreich; 175 Tests in 24 Dateien; Korpus 12/12 Fälle, 165/165 Felder, 16/16 Positionen; Fuzz 250/250, 4518/4518 Felder, 1841/1841 Positionen |
| `npm run check:native` | Clippy ohne Warnungen; 45 Rust-Tests bestanden |
| `dotnet test` PrintCore.Tests | 10 bestanden |
| `npm run check:xml` | 16/16 externe UBL-/CII-Prüfungen |
| `npm run check:pdf` | Musterrechnung: Mustang-XML bytegleich, veraPDF PDF/A-3b, Mustang Factur-X/ZUGFeRD |
| Isolierte Installer-Szenarien | `scripts/test-installer-lifecycle.ps1` bestanden, einschließlich Versionsmarke erst nach `RecordInstalledVersion` |
| Native Fenster (WP14) | Isoliertes Profil `erechnung-wp14-generalcheck`; Nachweis `artifacts/wp14-native-smoke.json` (2026-09-16T15:53:12.593Z) |
| Native Warteschlange/Neustart/Schließen | Isoliertes Profil `erechnung-wp7-generalcheck`; prepare, hartes Beenden, resume und close bestanden |
| Debug-Build | `npx tauri build --debug --no-bundle`; EXE unter `apps/desktop/src-tauri/target/debug/erechnung-desktop.exe` |
| `npm run release:gate` | Ohne Blocker. 9 bestanden, 3 ausstehend (DATEV-Prüfprogramm, cargo-audit, Produktionssignaturen). Nachweis `artifacts/release-gate.json` (2026-09-16T15:57:43.873Z) |

Die XML-/PDF-Prüfungen verwenden KoSIT 1.6.3, Mustang 2.26.0, veraPDF 1.28.2 und Temurin 21 mit XRechnung-Konfiguration 2026-01-31 laut [Validator-Manifest](../apps/desktop/src-tauri/resources/validators/manifest.json).

## Abnahmematrix

[acceptance-matrix.md](acceptance-matrix.md) wurde durch `npm run release:matrix` im Gate aktualisiert. Die native Fensterzeile ist mit dem WP14-Smoke **bestanden**. Ungeprüft bleiben:

- Update vom echten 0.2.2-Teststand
- DATEV-Testimport / unbeaufsichtigtes offizielles Prüfprogramm
- Produktionssignaturen
- Pilotbetrieb

Der vorhandene Druck-Smoke vom 06.09. (`artifacts/wp7-print-smoke.json`) wurde in diesem Lauf **nicht** gegen das echte Benutzerprofil wiederholt.

## Bewusste Grenzen

- Kein Commit. `target/` und Installer-Payload bleiben außerhalb von Git.
- Kein Installer, keine Deinstallation und kein Update auf dem echten Nutzerbestand.
- `cargo-audit` ist nicht installiert; `npm audit` ohne High/Critical in Produktionsabhängigkeiten.
- Diagnose speichert weiterhin nur Zähler und Versionen, keinen Rechnungsinhalt.
- Unabhängige fachliche Prüfung echter Kundenbelege und OCR über eine lokale Tesseract-Installation sind nicht Teil dieses Generalchecks.
