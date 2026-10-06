# WP7 – Posteingang und wiederherstellbare Entwürfe

Stand: 06.09.2026, Meilenstein 0.3.0. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- Eigener Arbeitsbestand mit SQLite und unveränderten Original-PDFs, getrennt vom Archiv.
- Dauerhafte Vorgangs-IDs und Revisionsvergleich; Wiederholung derselben Druckjob-ID bleibt idempotent.
- Geordnete Aufnahme von Deep Links und Ordnerabgleich; neue Jobs ersetzen keine aktive Rechnung.
- Automatische Sicherung nach 450 ms Eingabepause; Quellen und Analyseergebnisse werden mitgespeichert und beim Neustart nicht neu extrahiert.
- Normales Schließen und Rechnungswechsel sichern ausstehende Eingaben. Speicherfehler verhindern den Wechsel und bleiben sichtbar.
- Automatische Speicherung bestätigt keine Lernregeln. Bewusstes Speichern und erfolgreicher Export behalten ihre Lernwirkung.
- Alte JSON-Entwürfe lassen sich mit ausdrücklich ausgewählter Original-PDF übernehmen. Keine Löschung alter Dateien und keine ungeprüfte Übernahme alter Textmarkierungen.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| TypeScript-/UI-Tests (`npm run check`) | 63 bestanden |
| Referenzkorpus | 6/6 Fälle, 46/46 Felder, 7/7 Positionen |
| Synthetisches Korpus | 250/250 Rechnungen, 4.518/4.518 Felder, 1.841/1.841 Positionen |
| Rust (`cargo test --lib`) | 15 bestanden, davon 5 Workspace-Tests |
| Rust-Lint (`cargo clippy --all-targets -- -D warnings`) | Bestanden |
| Native Aufnahme einer PDF und zehn zusätzlicher Druckübergaben | 11 Vorgänge, aktive Eingabe unverändert, kein Lernen durch Autosave |
| Harter Abbruch und Neustart im isolierten Debug-Profil | Gespeicherte Eingabe und alle 11 Vorgänge wiederhergestellt |
| Sofort schließen vor Ablauf des Autosave-Timers | Eindeutig neue Eingabe in SQLite bestätigt, natives Fenster beendet |
| Posteingang/Rechnung/Archiv bei 1.380 × 900 und 900 × 650 | Kein übergreifender horizontaler oder vertikaler Seitenscroll |
| Fehlende/geänderte Original-PDF, Pfadgrenze, defekter Snapshot | Negative Tests bestanden |
| Veraltete Schreibrevision, verspätete Schreibantwort, simulierter Schreibfehler | Keine falsche Speicherbestätigung; Wiederholung und Wechsel-/Schließsperre getestet |
| Übernahme eines alten JSON-Entwurfs mit zugeordneter PDF | UI-Test bestanden |
| Gemeinsamer Installer und Update 0.2.2 → 0.3.0 | Exitcode 0, installierte Version bestätigt, bestehender Drucker unverändert übernommen |
| Echter Kaltstart über „E-Rechnung“ | Bestanden: `opened`, genau ein gespeicherter Vorgang, beide PDF-Hashes identisch |
| Vorhandene Nutzerdaten nach Update und Drucktest | Archivdatenbank und Vorlagengedächtnis bytegleich zum Stand vor dem Update |
| Konsolenfenster / Programmeinstieg | Installierte EXE verwendet PE-Subsystem 2 (Windows GUI), normaler Druckstart ohne Debug-Port |

Schreibfehler werden durch eine abweisende SQLite-Test-Triggerfunktion und abgewiesene native UI-Aufrufe simuliert. Eine physisch volle Platte, echte ACL-Verweigerung, Stromausfall und andere Rechner wurden damit nicht geprüft. Der harte Abbruchtest belegt die Wiederaufnahme bereits bestätigter Eingaben, nicht die Rettung noch nicht gespeicherter Tastatureingaben. Zehn erzeugte Übergabedateien testen die Aufnahme nach der Druckbrücke; der echte Drucktest wird separat aufgeführt.

## Nativen Test wiederholen

Debug-Build mit `npx tauri build --debug --config apps/desktop/src-tauri/tauri.conf.json --no-bundle` erstellen. Eine frische, absolute Testwurzel mit Präfix `erechnung-wp7-` verwenden. Nur für diesen Kindprozess `ERECHNUNG_TEST_ROOT` und `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` setzen, danach die Umgebung wiederherstellen. Normal installierte Apps vorher regulär schließen; niemals ungeprüft fremde Prozesse beenden.

1. `node scripts/smoke-workspace.mjs prepare <Testwurzel>` ausführen.
2. Nur den anhand seines vollständigen Pfads identifizierten Debug-Testprozess hart beenden und mit denselben Testvariablen neu starten.
3. `node scripts/smoke-workspace.mjs resume <Testwurzel>` ausführen.
4. `node scripts/smoke-workspace.mjs close <Testwurzel>` ausführen. Dieser Schritt prüft nach Schließen des Fensters den gespeicherten, einmaligen Testwert direkt in SQLite.

Für einen erneuten vollständigen Lauf eine neue Testwurzel verwenden. Release-Builds ignorieren `ERECHNUNG_TEST_ROOT`; diese dürfen nicht als isolierte Profile behandelt werden. Der normale Start erhält keinen Debug-Port.

Der separate echte Drucktest läuft mit `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/smoke-print-workspace.ps1` bei regulär geschlossener App. Er verwendet den installierten virtuellen Drucker und das normale Benutzerprofil, erzeugt genau einen synthetischen Beleg mit Namen `WP7 Drucktest <ID>` und lässt diesen erhalten. `verify-print-workspace.mjs` vergleicht Druck-PDF, Originalkopie, SQLite-Vorgang und gespeicherten Snapshot. Bericht: `artifacts/wp7-print-smoke.json`. Dieser Test bestätigt keine Lernregeln und erzeugt keine fertige E-Rechnung.

## Installer-Meilenstein

`artifacts/windows/E-Rechnungs-Assistent-0.3.0-x64-Setup.exe`, SHA-256 `1fd54ec97356590a11193a2959f7f6d2de8519076520f7613693bcf6a4ebce39`. Gebaut nach erfolgreichem TypeScript-/Corpus-/Rust-Gate mit `-SkipChecks -SkipPrinterBuild`; die unveränderten vorhandenen Druckerpakete wurden erneut auf Vollständigkeit und Signatur geprüft. Der Build führte TypeScript-Prüfung und Web-Build der Oberfläche selbst erneut aus. Der gemeinsame Installer ist lokal installiert und über den echten Druckpfad getestet. Weiterhin ein Entwicklungsinstaller mit Testzertifikat, keine Produktionssignierung.

## Bewusste Grenzen und nächste Arbeit

Keine automatische Bereinigung oder Archivierung unfertiger Vorgänge. Eine nach hartem Abbruch nicht referenzierte Originaldatei kann zurückbleiben. Der Arbeitsplatz ist kein Backup; Sicherung/Wiederherstellung folgt mit WP12. Fertige Archiveinträge wurden in diesem Paket nicht migriert. WP15 folgt als nächstes und führt den unveränderlichen Rechnungssnapshot samt fachlicher Identität für DATEV ein; die WP7-Speicherrevision ist allein noch keine formatübergreifende Exportidentität. DATEV selbst ist in 0.3.0 noch nicht enthalten.
