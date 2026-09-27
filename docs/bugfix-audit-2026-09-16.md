# Fehler- und UI-Prüfung – 16.09.2026

Geprüft wurde der vorhandene Arbeitsstand von Version 0.3.1 einschließlich der bereits uncommitteten Änderungen. Dieser Bericht beschreibt nur die zusätzlichen Korrekturen aus diesem Prüflauf. Vorhandene Änderungen wurden beibehalten; kein Commit und keine Installation wurden ausgeführt.

## Korrekturen

| Bereich | Befund und Änderung |
| --- | --- |
| Archiv | Beim Auswählen einer anderen Rechnung blieben Angaben und Dateiaktionen der vorherigen Rechnung sichtbar, bei einem Ladefehler sogar dauerhaft. Details werden beim Wechsel geleert und Aktionen zusätzlich an die ausgewählte Kennung gebunden. Ladezustand und Ladefehler sind erkennbar. |
| Rechnungsprüfung | Die Übernahme einer PDF-Markierung änderte Rechnungswerte, ohne die zuvor erteilte Bestätigung der Originaltreue zurückzusetzen. Sie setzt jetzt wie eine manuelle Bearbeitung die Bestätigung und alte externe Prüffehler zurück. |
| PDF-Import | Während `File.arrayBuffer()` lief, konnte eine zweite Datei geöffnet oder abgelegt werden. Eine unmittelbare Importsperre und konsistent gesperrte Bedienelemente verhindern konkurrierende Importe und Änderungen am bisherigen Entwurf während des Wechsels. |
| Kanzleiangaben | Das Ereignis nach einer älteren Speicherung konnte neuere Kontoeingaben durch erneutes Laden überschreiben. Lokale Änderungen und deren Revision werden nun vor und nach dem Nachladen geprüft. |
| Kanzleiangaben | Nach einem Speicherfehler blieb „wird gespeichert“ stehen und die Navigation war ohne Wiederholungsmöglichkeit gesperrt. Der Fehlerzustand wird jetzt angezeigt und „Erneut speichern“ sichert die aktuellen Eingaben. Auch ein Ladefehler wird als solcher bezeichnet. |
| Kanzleiexport | Die Auswahlliste zeigte alle Rechnungsbeträge als EUR an. Sie verwendet nun die tatsächliche Archivwährung. Die fachliche Beschränkung des Exports auf EUR bleibt bestehen. |
| Wiederherstellung | Eine alte Freigabe blieb beim Ändern von Dateipfad/Kennwort oder nach einer fehlgeschlagenen erneuten Prüfung erhalten. Sie wird nun verworfen; normale Statusabfragen reaktivieren sie nicht. Der Zeitpunkt der geprüften Sicherung wird angezeigt. |
| Tastaturbedienung | Die Dateiauswahl war durch `display: none` aus der Tab-Reihenfolge entfernt. Sie bleibt nun fokussierbar und hat einen sichtbaren Fokusrahmen. Der Einstellungsdialog erhält beim Öffnen den Fokus, hält Tab/Shift+Tab im Dialog und stellt den vorherigen Fokus beim Schließen wieder her. |
| Darstellung | Doppelte Überschrift „Sicherung erstellen“ entfernt. Kopfzeile, Einstellungsnavigation, Archivkennzeichnung und Dateiaktionen können bei wenig Platz umbrechen. Bei niedrigen Fenstern bleibt der Archivinhalt über Scrollen erreichbar. Deaktivierte Dateiauswahlen sind sichtbar abgesetzt. |

## Verifikation

- `npm run check`: 185 Tests in 26 Dateien bestanden; Core-, Desktop- und Test-Typprüfung bestanden. Darunter zehn neue Regressionstests für die genannten Bedienabläufe.
- Korpus: 12/12 Fälle, 165/165 Felder, 16/16 Positionen.
- Fuzz: 250/250 Rechnungen, 4518/4518 Felder, 1841/1841 Positionen.
- `npm run check:native`: Clippy ohne Warnungen, 45 Rust-Tests bestanden.
- `dotnet test drucker/tests/PrintCore.Tests/PrintCore.Tests.csproj --no-restore`: 10 Tests bestanden.
- `npm run desktop:web:build`: Produktionsbuild erfolgreich; Vite meldet lediglich die Größe des PDF.js-Chunks über 500 kB.
- `npm run check:xml`: 22/22 externe UBL-/CII-Prüfungen mit KoSIT und Mustang bestanden.
- `npm run check:pdf`: bytegleicher Mustang-XML-Extrakt, veraPDF-PDF/A- und Mustang-Factur-X/ZUGFeRD-Prüfung bestanden.
- `git diff --check`: keine Whitespace-Fehler; Git meldet lediglich die konfigurierte LF/CRLF-Konvertierung.

Die neu ergänzten Tests liegen in `test/archive-ui.test.tsx`, `test/datev-ui.test.tsx`, `test/learning-ui.test.tsx` und `test/settings-ui.test.tsx`. Sechs Regressionen wurden zunächst gegen den unveränderten Anwendungscode reproduziert und anschließend mit den Korrekturen erfolgreich geprüft.

## Grenzen

Die native Computer-Use-Verbindung war nicht verfügbar (fehlende Native Pipe); auch die Browser-Inventarliste enthielt keinen verfügbaren Browser. Deshalb wurde in diesem Lauf keine visuelle Screenshot-Abnahme durchgeführt. Bedienlogik und Tastatur-Fokus wurden mit React-/DOM-Tests geprüft; die CSS-Anpassungen sind anhand des vorhandenen Layoutcodes umgesetzt. Ein echter Druckauftrag, Installer-/Updateablauf und Produktionssignaturen wurden in diesem Lauf nicht erneut geprüft.
