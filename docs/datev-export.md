# DATEV-Export – Entwicklungsstand

Stand: 7. September 2026. Entwicklungsstand, keine freigegebene DATEV-Integration und kein Nachweis eines erfolgreichen Imports in DATEV Rechnungswesen.

Der TypeScript-Adapter erzeugt EXTF-Buchungsstapel (700, Kategorie 21, Version 13) mit Windows-1252-Codierung und eine DATEV-`document.xml` (Belegtransfer v6.0). Jede Rechnung erhält eine stabile GUID: Sie steht in der EXTF-Spalte Beleglink als `BEDI "<GUID>"` und im Belegpaket. Rust packt PDF, Rechnungs-XML und `document.xml` unverändert in `Belege.zip`. Die Oberfläche ist über das Archiv erreichbar. Voraussetzung sind ausdrücklich bestätigte Kanzlei-, Betriebs- und Kontierungsangaben. Unterstützt werden zunächst normale deutsche Ausgangsrechnungen in EUR mit 7/19 Prozent Umsatzsteuer und Sollversteuerung.

Neue Archiveinträge enthalten einen vollständigen, unveränderlichen Rechnungsstand mit Dokumentkennung, Quellrevision und Original-Hash. Dieser Stand wird in die bestehende Hashkette eingebunden. Ältere Einträge ohne Rechnungsstand bleiben für diesen Export gesperrt. XML- und PDF-Ausgaben desselben Rechnungsstands werden zusammengefasst; mögliche Wiederholungen erfordern eine ausdrückliche Begründung.

Rust speichert vorbereitete Exporte samt unveränderten Dateiinhalten zunächst in einem SQLite-Journal. Erst nach erfolgreicher Dateiausgabe wird der Vorgang abgeschlossen. Unterbrochene Vorgänge können fortgesetzt werden. Bereits vorhandene veränderte Dateien werden nicht überschrieben. Das Belegpaket ist eine lokale Datei für DATEV Belegtransfer, keine Übertragung an eine Kanzlei oder an DATEV-Rechenzentren.

## Prüfstand und offene Abnahme

- 25 TypeScript-Tests für den DATEV-Adapter; gesamte TypeScript-/UI-Suite zuletzt über 100 Tests.
- Rust-Journal- und Archivtests im nativen Paket.
- Oberfläche über das Archiv erreichbar („Für die Steuerkanzlei exportieren“).
- Offizielles DATEV-Prüfprogramm und echter DATEV-Testimport mit der Kanzlei bleiben erforderlich.
- Kein neuer Installer veröffentlicht; App-Version weiterhin 0.3.0.

Formatquellen: [DATEV-Format](https://developer.datev.de/de/file-format/details/datev-format) und [DATEV XML-Schnittstelle online, Verwaltungsdatendatei](https://developer.datev.de/de/file-format/details/datev-xml-interface-online/format-specification-/administrative-data-file). `node scripts/fetch-datev-reference.mjs --tools` lädt die EXTF-Referenz und die angebotenen Prüfwerkzeuge lokal nach. Downloads und erzeugte Testartefakte gehören nicht ins Repository.
