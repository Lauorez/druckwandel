# Architekturprüfung vom 4. September 2026

## Ergebnis

Die Zielarchitektur bleibt ein TypeScript-Fachkern in einer Tauri-Anwendung. Rust ist die schmale Vertrauensgrenze für lokale Dateien, SQLite, Prozessstarts und Deep Links. Die Windows-Druckannahme bleibt ein getrenntes MSIX-Paket. Es gibt keine zweite Rechnungslogik in Rust oder .NET.

| Bereich | Verantwortung | Technologie |
| --- | --- | --- |
| Fachkern | Modell, Berechnung, Validierung, UBL und CII | TypeScript |
| Erkennung | PDF-Text, Layout, Tabellen, Vorlagengedächtnis | TypeScript |
| Oberfläche | Prüfung, Korrektur, Ausgabe und Archivansicht | React im Tauri-WebView |
| Lokale Systemgrenze | sichere Pfade, atomare Entwürfe, Archiv, Deep Links | Rust/Tauri |
| Windows-Druckannahme | OXPS/PDF übernehmen, konvertieren, an Tauri übergeben | .NET/WinRT und eine minimale WinUI-Druckbrücke |

## Entscheidung zur WinUI-Druckbrücke

Die WinUI-Komponente bleibt erhalten, aber nur als Adapter des Windows-Vertrags `windows.printSupportJobUI`. Der isolierte Virtual-Printer-Background-Task kann über `PrintWorkflowUILauncher.LaunchAndCompleteUIAsync` diesen Vordergrund-Endpunkt aktivieren. Die Brücke kopiert den bereits konvertierten Auftrag atomar in den gemeinsamen Druckeingang, öffnet eine UUID-basierte URI und beendet sich. Nur wenn die Übergabe fehlschlägt, bleibt ihre kleine Wiederholungsoberfläche sichtbar.

Ein Entfernen würde nicht bloß ein überflüssiges Fenster beseitigen, sondern den offiziell vorgesehenen Vordergrund-Endpunkt des Windows-Druckworkflows. Eine Zusammenlegung mit Tauri würde außerdem Paketidentität, WinRT-Aktivierung und Fachanwendung unnötig koppeln. Referenzen: [Microsoft Print Support App v4 design guide](https://learn.microsoft.com/windows-hardware/drivers/devapps/print-support-app-v4-design-guide), [Microsoft Print Support App sample](https://github.com/microsoft/print-oem-samples).

## Behobene Architekturprobleme

- Der Druckeingang bildet beim Start zuerst eine Bestandsaufnahme. Alte PDFs werden nicht mehr als neuer Druckauftrag geöffnet. Ein gezielt per Start-Link übergebener Job hat jedoch Vorrang vor dieser Bestandsaufnahme; andernfalls würde ausgerechnet der Job, der die App gestartet hat, als bereits bekannt verworfen. Nachfolgende Aufträge werden geordnet verarbeitet; parallele Abfragen können sich nicht mehr überholen. Die Abgleichlogik besitzt eigene Regressionstests.
- Fertige Dateien haben nur noch einen nativen Speicherweg: Ausgabe und Archivierung laufen gemeinsam durch `save_and_archive_invoice`. Die alten parallelen XML- und Binärkommandos sowie ein wirkungsloser zusätzlicher Inbox-Aufruf wurden entfernt. Nur Entwürfe besitzen weiterhin ein eigenes eng begrenztes Kommando.
- CII und Hybrid-PDF verwenden beim ZUGFeRD-Export exakt dieselben einmal erzeugten XML-Bytes.
- PDF.js und `pdf-lib` liegen nicht mehr im Startblock. PDF-Analyse und Export werden erst bei Bedarf geladen; der JavaScript-Startblock sank im Release-Build von ungefähr 1,28 MB auf 286 KB.
- Geld- und Mengenwerte durchlaufen auch bei Anzeige und Extraktionsnormalisierung keine binäre JavaScript-Fließkommaarithmetik mehr.
- Kalenderdaten werden als echte Daten geprüft; Werte wie `2026-02-29` werden abgelehnt.
- Das Vorlagengedächtnis prüft jetzt auch Typ und Obergrenze seiner Tabellenregeln auf der nativen Speichergrenze.
- Die Archivsuche liefert Gesamtzahl und Seiten mit jeweils höchstens 100 Einträgen. Die frühere stille Grenze von 500 Treffern wurde entfernt.
- Suche und Filter laden nur noch die Trefferliste neu; der unveränderte globale Archivstatus wird nicht mehr bei jedem Tastendruck ein zweites Mal aus SQLite gelesen.
- Fehler beim Öffnen von Archivordnern und Prüfberichten erscheinen in der Oberfläche statt als unbehandelte Promise.
- Technische Hashwerte wurden aus der für nichttechnische Anwender gedachten Detailansicht entfernt. Sie bleiben vollständig in SQLite gespeichert und werden von der Archivprüfung weiterhin ausgewertet.
- Die veraltete ZUGFeRD-Profilkennung wurde auf `urn:cen.eu:en16931:2017` aktualisiert und mit Mustang 2.26.0 gegen den ZUGFeRD-2.5-Prüfsatz verifiziert.
- Drei alte MSIX-Testpakete, eine entpackte Paketkopie und zwei unreferenzierte Microsoft-Beispielstände wurden aus dem Arbeitsbaum entfernt. Die bytegleiche zweite Kopie des MVP-Projektplans wurde ebenfalls entfernt.

## Bewusst getrennte oder begrenzte Teile

- `erechnung-review://` und die Dateiendung `.review.json` bleiben als versionierter Drahtvertrag bestehen. Eine kosmetische Umbenennung würde installierte Protokollhandler und vorhandene Druckjobs ohne fachlichen Gewinn brechen.
- Die optionale lokale Ed25519-Bestätigung erhöht die Manipulationserkennung, ist aber kein externer Vertrauensanker. Archiv, Datenbank und lokaler Schlüssel müssen gemeinsam gesichert werden.
- Das Einbetten von CII konvertiert ein beliebiges Eingabe-PDF nicht zu PDF/A-3. Die Anwendung behauptet deshalb keine ungeprüfte PDF/A-Konformität. Eine produktive Freigabe braucht zusätzlich einen vollständigen PDF/A-Lauf mit veraPDF.
- Die Druckbrücke setzt Windows 11 Build 26100 voraus. Der TypeScript-Kern und Tauri bleiben plattformübergreifend; eine macOS-Druckannahme wäre ein eigener Adapter vor demselben Inbox-Vertrag.

## Verbindliche Prüfgates

```powershell
npm run check
npm run desktop:web:build
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
powershell -NoProfile -ExecutionPolicy Bypass -File drucker/scripts/test.ps1
npm run desktop:build
npm run printer:build
```

Für Formatänderungen kommen KoSIT für XRechnung und Mustang für ZUGFeRD/Factur-X hinzu. Ein Prüfergebnis darf nur dem Profil zugerechnet werden, dessen Szenario tatsächlich gewählt wurde.
