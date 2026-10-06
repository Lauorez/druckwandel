# Dokumentation

## Für Anwender

- [Benutzerhandbuch](benutzerhandbuch.md): Rechnungen übernehmen, prüfen, ausgeben, archivieren, sichern
- [Kurzanleitung für den Pilotbetrieb](pilot-guide.md)
- [Windows-Vorführpaket](windows-demo-installation.md): Installation, Zertifikatfreigabe und Grenzen des aktuellen Builds
- [DATEV-Export](datev-export.md): Entwicklungsstand und Grenzen

## Für Entwickler

- [Architektur](architecture.md): Komponenten, Datenfluss und Sicherheitsgrenzen der Desktop-Anwendung
- [Korpus und Qualitätsgate](corpus.md): Referenzkorpus, synthetische Rechnungen, sicheres Anonymisieren eigener Belege
- [Gebündelte Komponenten](components.md): Versionen, Herkunft und Lizenzen (erzeugt von `npm run release:inventory`)
- [Gebündelte Prüfwerkzeuge](../apps/desktop/src-tauri/resources/validators/README.md)
- [Virtueller Drucker](../printer/README.md) mit [Architektur](../printer/docs/architecture.md), [Testplan](../printer/docs/testing.md) und [Notizen zur Windows-Print-API](../printer/docs/windows-print-api-notes.md)

## Abnahme und Qualität

- [Abnahmematrix](acceptance-matrix.md): was auf welchem System geprüft ist und was nicht (erzeugt von `npm run release:matrix`)
- Abnahmeprotokolle der Arbeitspakete (WP) aus der [Roadmap](planning/roadmap-1.0.md):
  - [WP7: Posteingang und wiederherstellbare Entwürfe](acceptance/wp7-acceptance.md)
  - [WP8: Verbindliche lokale XML-Prüfung](acceptance/wp8-acceptance.md)
  - [WP9: Hybrid-PDF und PDF/A-3](acceptance/wp9-acceptance.md)
  - [WP10: Erkennungsqualität und kontrollierbares Lernen](acceptance/wp10-acceptance.md)
  - [WP11: Rechnungsumfang](acceptance/wp11-acceptance.md)
  - [WP12: Sicherung, Wiederherstellung und Absturzbehandlung](acceptance/wp12-acceptance.md)
  - [WP13: Wartbare und sichere Auslieferung](acceptance/wp13-acceptance.md)
  - [WP14: Release Candidate](acceptance/wp14-acceptance.md)
- Prüfberichte: [Cleanup 11.09.2026](audits/cleanup-audit-2026-09-11.md), [Generalcheck 16.09.2026](audits/cleanup-audit-2026-09-16.md), [Fehler- und UI-Prüfung 16.09.2026](audits/bugfix-audit-2026-09-16.md)
- Installationsberichte: [0.3.2](releases/installation-0.3.2-2026-09-27.md), [0.3.3](releases/installation-0.3.3-2026-09-27.md), [0.3.4](releases/installation-0.3.4-2026-09-27.md)

## Entscheidungen

- [Architekturprüfung September 2026](decisions/2026-09-architecture-audit.md)
- [Wahl des PDF/A-Konverters](decisions/pdfa-converter.md)

## Planung

- [Roadmap bis 1.0](planning/roadmap-1.0.md)
- [Ursprünglicher MVP-Projektplan](planning/mvp-projektplan.md) (historisch)
