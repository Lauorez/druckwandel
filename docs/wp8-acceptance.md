# WP8 – Verbindliche lokale XML-Prüfung

Stand: 07.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- Gemeinsames Ergebnisformat für Status, Fehler, Regelversion und Artefakt-Hashes; Exitcode 0 allein reicht nicht.
- Native Fertigstellung prüft exakt die vorbereiteten XML-/PDF-Bytes mit gebündeltem KoSIT (XRechnung) bzw. Mustang (ZUGFeRD-XML), ohne Java aus PATH und ohne HTTP-Daemon.
- Ein kurzlebiger Prüfbeleg bindet Vorgang, Entwurfrevision, Snapshot und Dateihashes. Änderungen danach machen ihn ungültig.
- Ohne erfolgreiche Prüfung entsteht keine fertige Ausgabe; Entwürfe bleiben speicherbar. Wiederholung derselben Revision erzeugt keinen zweiten Archiveintrag.
- Neue Archiveinträge (Kettenvariante v3) speichern Prüfbericht und Bericht-Hash. Ältere Einträge ohne Bericht bleiben unverändert und gelten nicht rückwirkend als geprüft.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| TypeScript-Berichtparser (gültig, ungültig, fehlender Bericht, Reject trotz Exit 0, Timeout) | `test/validators.test.ts` |
| Rust-Berichtparser, Ticketbindung, Archiv v3 und Idempotenz | `cargo test` in `apps/desktop/src-tauri` |
| Feldzuordnung deutscher Angaben | Parser-Tests plus UI-Sprung zur betroffenen Stelle |

Echte KoSIT-/Mustang-Läufe gegen Fixtures erfordern das Paket aus `npm run validators:fetch` zuzüglich XRechnung-Konfiguration und privater JRE laut `apps/desktop/src-tauri/resources/validators/README.md`. Ohne dieses Paket lehnt die native Fertigstellung ab.

## Bewusste Grenzen

PDF/A-Konvertierung und veraPDF sind in WP9 beschrieben. DATEV-Stapel und ältere Rechnungsrevisionen wurden nicht nachträglich als geprüft gekennzeichnet. Eine technische XML-Prüfung ist keine sachliche oder steuerliche Richtigkeitsgarantie.
