# WP14 – Release Candidate und dokumentierte Freigabe

Stand: 16.09.2026. Lokale Entwicklungsabnahme, **keine 1.0-Freigabe** und keine Kundenfreigabe.

## Umsetzung

- Die Abnahmematrix in [acceptance-matrix.md](../acceptance-matrix.md) nennt Datum, App-/Standardversion, Betriebssystem, Testdaten und Ergebnis. Ungeprüfte Kombinationen bleiben ungeprüft; lokale Unit-Tests ersetzen keinen Druck-, Update-, DATEV- oder Pilotnachweis.
- Native Abläufe im echten Tauri-Fenster: `scripts/smoke-workspace.mjs` (Import, Warteschlange, Neustart, Schließen) und `scripts/smoke-release.mjs` (Archiv, DATEV-Ansicht, Diagnose/Sicherung, Escape, 100/150/200 %). Nur isolierte `erechnung-wp*`-Profile. Nachweis: `artifacts/wp14-native-smoke.json`, sonst im Release-Gate **pending**.
- Drucken bei geschlossener App bleibt `scripts/smoke-print-workspace.ps1` auf dem installierten Drucker; ohne `artifacts/wp7-print-smoke.json` ungeprüft.
- Bedienbarkeit: sichtbarer Tastaturfokus, `aria-current` in der Navigation, Escape schließt Einstellungen (Fenster und Overlay), Speicherstatus, Ladehinweise, Export erst nach Bestätigung der Originalangaben, DATEV-Wiederholung nur mit Bestätigung und Begründung.
- Pilotunterlage: [pilot-guide.md](../pilot-guide.md).

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Archivsuche, Archivprüfung, DATEV-Dublettenschutz in der UI | `test/release-ui.test.tsx` |
| Beide Exportwege erst nach Originalbestätigung | `test/review-ui.test.tsx` |
| Tastatur Escape, Sicherungserinnerung | `test/release-ui.test.tsx` |
| Matrix behauptet kein Bestanden ohne Nachweis | `test/acceptance-matrix.test.ts`, `npm run release:matrix` |
| Native Fenster-/Skalierungsprüfung | `scripts/smoke-release.mjs` / `scripts/smoke-native-window.ps1` (ohne Report: pending) |
| Warteschlange, Neustart, Schließen | `scripts/smoke-workspace.mjs` (WP7, isoliert) |
| XML/PDF-Regressionen | `npm run check:xml` / `check:pdf` sofern JRE vorhanden |

Technischer Abschluss dieses Pakets: vereinbarte WP7–WP13 plus diese RC-Unterlagen. Keine bekannten ungeprüften stillen Datenverluste in den automatisierten Negativtests. Unabhängige XML/PDF-Prüfung und Wiederherstellung sind in WP8/WP9/WP12 nachgewiesen, soweit die jeweiligen Gates liefen.

## Bewusste Grenzen (Kundenfreigabe 1.0 bleibt offen)

Produktionssignaturen, Update vom echten 0.2.2-Teststand, DATEV-Testimport, Pilotbetriebe und unabhängige fachliche/Sicherheitsprüfung sind **ungeprüft**. Lokale Tests ersetzen das nicht.
