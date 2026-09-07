# DATEV-Export – Entwicklungsstand

Stand: 7. September 2026. Implementierung in Arbeit, keine freigegebene DATEV-Integration und kein Nachweis eines erfolgreichen Imports in DATEV Rechnungswesen.

Der TypeScript-Adapter erzeugt EXTF-Buchungsstapel (700, Kategorie 21, Version 13) mit Windows-1252-Codierung. Die Oberfläche ist über das Archiv erreichbar. Voraussetzung sind ausdrücklich bestätigte Kanzlei-, Betriebs- und Kontierungsangaben. Unterstützt werden zunächst normale deutsche Ausgangsrechnungen in EUR mit 7/19 Prozent Umsatzsteuer und Sollversteuerung.

Neue Archiveinträge enthalten einen vollständigen, unveränderlichen Rechnungsstand mit Dokumentkennung, Quellrevision und Original-Hash. Dieser Stand wird in die bestehende Hashkette eingebunden. Ältere Einträge ohne Rechnungsstand bleiben für diesen Export gesperrt. XML- und PDF-Ausgaben desselben Rechnungsstands werden zusammengefasst; mögliche Wiederholungen erfordern eine ausdrückliche Begründung.

Rust speichert vorbereitete Exporte samt unveränderten Dateiinhalten zunächst in einem SQLite-Journal. Erst nach erfolgreicher Dateiausgabe wird der Vorgang abgeschlossen. Unterbrochene Vorgänge können fortgesetzt werden. Bereits vorhandene veränderte Dateien werden nicht überschrieben. Die optionale Belegablage ist kein DATEV-Belegtransfer; es erfolgt keine Übertragung an eine Kanzlei.

## Prüfstand und offene Abnahme

- 24 neue TypeScript-Tests bestanden; gesamte TypeScript-/UI-Suite zuletzt 87 Tests bestanden.
- Sechs neue Rust-Journaltests bestanden; zusammen mit den bisherigen Tests zuletzt 21 Rust-Tests bestanden.
- Ein zusätzlicher Test zur Migration und signierten Archivkette wurde danach ergänzt und muss noch ausgeführt werden.
- Oberflächen-Build erfolgreich; vollständiger nativer End-to-End-Test der neuen Ansicht steht noch aus.
- Offizielles DATEV-Prüfprogramm heruntergeladen, aber noch nicht ausgeführt. Echter DATEV-Testimport mit der Kanzlei bleibt erforderlich.
- Kein neuer Installer veröffentlicht; installierter Stand weiterhin 0.3.0.

Formatquelle: [DATEV Developer Portal](https://developer.datev.de/de/file-format/details/datev-format). `node scripts/fetch-datev-reference.mjs --tools` lädt die Referenz und die angebotenen Prüfwerkzeuge lokal nach. Downloads und erzeugte Testartefakte gehören nicht ins Repository.
