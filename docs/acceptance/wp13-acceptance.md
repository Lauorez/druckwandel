# WP13 – Wartbare und sichere Auslieferung

Stand: 16.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- `npm run release:gate` führt TypeScript/UI/Korpus, Rust-Clippy/-Tests, .NET-Druckkern, KoSIT/Mustang, Hybrid-PDF/A, isolierte Installer-Szenarien, `npm audit --omit=dev --audit-level=high` und optional `cargo audit` zusammen. Fehlschläge blockieren. Ausstehende Schritte gelten nicht als bestanden.
- Das offizielle DATEV-Prüfprogramm bleibt `pending`, solange `ERECHNUNG_DATEV_CHECKER` nicht gesetzt ist und ausgeführt wurde. Adaptertests und eine lokal heruntergeladene ZIP sind kein Ersatz.
- `node scripts/inventory-components.mjs` schreibt Herkunft, Version und Lizenz der gebündelten Bestandteile nach `docs/components.md` und `artifacts/components.json`.
- Produktionsbuilds (`installer:windows -SigningMode Production`) dürfen das Gate nicht überspringen. Signaturprüfung lehnt ungültige und selbstsignierte Zertifikate ab. Das Produktionszertifikat bleibt eine externe Voraussetzung.
- Vor einem Update sichert `UpdateGuard.ps1` Entwürfe, Archivdatenbank, Vorlagengedächtnis und Schlüssel und schreibt `WIEDERHERSTELLUNG.txt`. Eine kleinere Versionsnummer als die installierte wird abgewiesen. Die Deinstallation darf `Dokumente\E-Rechnungsarchiv` nicht löschen.
- Deep Links akzeptieren nur `erechnung-review://print-job/<UUID>`. Native Pfade, JSON/XML-Größen, Junctions über Canonicalize und Prozessargumente der Prüfer bleiben begrenzt. Lang laufende Prüfungen sind abbrechbar, starten ohne Konsolenfenster und schreiben keine Rechnungsinhalte in allgemeine Fehlerlogs.
- Einstellungen → Diagnose zeigt einen Bericht ohne PDF/XML, Bankdaten und persönliche Dateipfade. Speichern nur nach Vorschau und Bestätigung; kein automatischer Versand.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Release-Gate-Skripte, DATEV nicht automatisch bestanden, Produktion ohne Gate/Testzertifikat | `test/release-gate.test.ts`, `test/windows-installer.test.ts` |
| Isolierte Update-/Datenprüfung (keine Abwärtsinstallation, Snapshot, Archiv bleibt) | `scripts/test-installer-lifecycle.ps1` |
| Diagnosebericht ohne Pfade/Rechnungsinhalte | `diagnostics::tests::omits_paths_and_invoice_payloads` |
| Diagnose-UI: Vorschau, Bestätigung, Speichern | `test/settings-ui.test.tsx` |
| Deep Links ohne Dateipfade | `test/print-inbox.test.ts`, `print_job_ids_reject_paths_and_deep_link_payloads` |
| Komponentenliste | `docs/components.md` |
| Portables CI-Workflow | `.github/workflows/release-gate.yml` (`ubuntu-latest`, `--portable`) |

Nicht Teil dieser Abnahme: Neuinstallation und Update vom gesicherten 0.2.2-Teststand auf einem echten Windows-Rechner, Deinstallation/Neuinstallation mit Nutzerdaten, Druckertest als normaler Benutzer, Produktionssignaturen und unbeaufsichtigtes DATEV-Prüfprogramm. Isolierte Tests ersetzen diese Hardwareprüfungen nicht und dürfen den echten Nutzerbestand nicht zurücksetzen.

## Bewusste Grenzen

Online-Autoupdate ist für 1.0 nicht vorgesehen. Ein Windows-CI-Runner wird nicht gebunden. `npm run release:gate -- --portable` darf Rust, .NET, Validatoren und Installer nicht als bestanden ausweisen. Testzertifikate sind kein Release.
