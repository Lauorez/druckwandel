# Abnahmematrix 0.3.1

Stand: 2026-09-27. Keine 1.0- oder Kundenfreigabe.

Nicht verfügbare Kombinationen stehen als **ungeprüft**. Ein bestandener Unit-Test auf dem Entwicklungsrechner gilt nicht als Nachweis für Druck, Installer-Update, DATEV-Import oder Pilotbetrieb.

| Ablauf | Testdaten | Betriebssystem | App | Standard | Ergebnis | Nachweis |
| --- | --- | --- | --- | --- | --- | --- |
| TypeScript, UI, Korpus, DATEV-Adapter, Sicherung, Diagnose | Referenzkorpus und synthetische Fixtures | Entwicklung (dieser Arbeitsbaum) | 0.3.1 | EN 16931 / XRechnung 3.0.2 | bestanden | npm test / npm run check. Kein Zielsystemtest. |
| Rust-Archiv, Workspace, DATEV-Journal, Backup, Diagnosegrenzen | isolierte Temp-Profile | Entwicklung (dieser Arbeitsbaum) | 0.3.1 | — | bestanden | cargo test --lib. Kein Zielsystemtest. |
| Unabhängige XML- und Hybrid-PDF/A-Prüfung | test/fixtures und Musterrechnung | Windows mit gebündelter JRE | 0.3.1 | XRechnung 3.0.2, ZUGFeRD EN 16931, PDF/A-3b | lokal ausführbar | npm run check:xml / check:pdf. Nicht aus Vitest ableiten, wenn die JRE fehlt. |
| Echtes Tauri-Fenster: Posteingang, Archiv, DATEV, Diagnose, Tastatur, 100/150/200 % | isoliertes erechnung-wp*-Profil | Windows 11 Debug-Build | 0.3.1 | — | bestanden | artifacts/wp14-native-smoke.json (2026-09-16T15:53:12.593Z) |
| Drucken bei geschlossener App | WP7 Drucktest <ID> | Windows 11 mit Drucker E-Rechnung | 0.3.1 | — | bestanden | artifacts/wp7-print-smoke.json (2026-09-27T13:30:17.046Z) |
| Isolierte Update-/Datenprüfung, keine Abwärtsinstallation | temporäre Dummy-SQLite | Windows | 0.3.1 | — | bestanden | scripts/test-installer-lifecycle.ps1. Kein Update vom echten 0.2.2-Teststand. |
| Update vom gesicherten 0.2.2-Teststand, Deinstallation, Neuinstallation | gesicherter Nutzerbestand | bereitgestelltes Windows 11 | 0.3.1 | — | ungeprüft | Hardwareabnahme. Nicht aus dem isolierten String-/Snapshot-Test ableiten. |
| DATEV-Testimport des beworbenen EXTF-Umfangs | Kanzleiprofil der Pilotkanzlei | DATEV Rechnungswesen | 0.3.1 | EXTF 700/21/13, Belegtransfer v6.0 | ungeprüft | Offizielles Prüfprogramm und Kanzleiimport bleiben extern. |
| Produktionssignaturen Anwendung, Helfer, Drucker, Installer | — | Windows | 0.3.1 | Authenticode | ungeprüft | Testzertifikate sind kein Release. --require-production-signatures. |
| Pilotbetrieb und ausgewertetes Feedback | echte Belege der Pilotbetriebe | Zielsysteme der Pilotbetriebe | 0.3.1 | vereinbarter Rechnungsumfang | ungeprüft | Betriebe, Einverständnisse und Rückmeldungen sind nicht Teil dieser lokalen Abnahme. |

Maschinenlesbar: `artifacts/acceptance-matrix.json` (gitignoriert). Erzeugen: `npm run release:matrix`.
