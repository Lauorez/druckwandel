# Benutzerhandbuch

Dieses Handbuch beschreibt die Bedienung der installierten Windows-Anwendung. Installation und Entwicklung stehen im [README](../README.md), eine Kurzfassung für Pilotbetriebe in [pilot-guide.md](pilot-guide.md).

## Erster Durchlauf mit der Musterrechnung

1. Setup normal starten, nicht über „Als Administrator ausführen“. Falls Windows dem Entwicklungszertifikat noch nicht vertraut, fragt das Setup einmalig nach Administratorrechten (siehe [Windows-Vorführpaket](windows-demo-installation.md)).
2. In den **Einstellungen** unter **Sicherung** eine erste Sicherung auf ein anderes Laufwerk anlegen.
3. Die Musterrechnung (`npm run demo:invoice` erzeugt `artifacts\demo\muster-rechnung.pdf`) über **Rechnung öffnen** laden.
4. Angaben prüfen, die Übereinstimmung mit dem Original bestätigen und eine Ausgabe speichern.
5. Den Eintrag im **Archiv** ansehen und optional den Beleg aus einer beliebigen Anwendung auf den Drucker **E-Rechnung** drucken.

## Rechnungen übernehmen

### Über den Drucker „E-Rechnung“

Das Setup richtet den virtuellen Drucker **E-Rechnung** ein. Wird aus Word, Excel, einem Browser oder Branchensoftware darauf gedruckt, erscheint kein Speichern-unter-Dialog. Die Anwendung darf geschlossen sein; sie öffnet sich mit dem neuen Beleg. Neue Druckaufträge ersetzen eine gerade geöffnete Rechnung nicht, sondern landen im **Posteingang**.

Technisch übergibt die Druckbrücke drei Dateien in `Dokumente\E-Rechnung Druckeingang`:

```text
<UUID>.pdf             vollständiges Druck-PDF
<UUID>.printjob.json   schemaVersion 1 und Druckjob-Metadaten
<UUID>.review.json     Bestätigung von Druckwandel: opened oder failed
```

### Ohne virtuellen Drucker

1. In einer beliebigen Anwendung **Microsoft Print to PDF** wählen.
2. Als Ziel den Ordner `Dokumente\E-Rechnung Druckeingang` wählen.
3. Die App übernimmt die neue PDF beim nächsten Abgleich (etwa alle 1,5 Sekunden) in den Posteingang.

Beim allerersten Einrichten bereits vorhandene Dateien werden als ältere Dateien zur ausdrücklichen Übernahme angeboten. PDFs lassen sich auch direkt über **Rechnung öffnen** laden.

### Gescannte Dokumente

Seiten ohne Textlayer werden mit `OCR_REQUIRED` markiert und müssen manuell erfasst werden. Für Entwickler bietet `npm run extract:pdf -- <datei> --ocr` einen lokalen Tesseract-Fallback (Tesseract und Sprachdaten `deu`/`eng` müssen über `PATH` erreichbar sein).

## Prüfen und korrigieren

Erkannte Werte lassen sich direkt korrigieren. Positionen können ergänzt und entfernt werden. Beträge erscheinen deutsch formatiert, zum Beispiel 1.234,56; intern bleiben sie exakte Dezimalwerte.

Unter **Belegart** wird zwischen Rechnung, Gutschrift und Rechnungskorrektur gewählt; Gutschriften und Korrekturen brauchen die Nummer der Ursprungsrechnung. Der Steuerfall (Standardsteuer, Reverse Charge, steuerfrei, innergemeinschaftliche Lieferung, steuerbare 0 %) wird ausdrücklich gewählt. Nachlässe und Zuschläge auf Positions- oder Belegebene sowie Abschlags-, Anzahlungs- und Schlussrechnungen werden ebenfalls unterstützt.

Mit **Im PDF markieren** neben einem Feld lässt sich die Quelle gezielt bestimmen: Text anklicken oder einen Rahmen darum ziehen, den markierten Wert prüfen und **Übernehmen** wählen. Der Wert wird dabei gegen die Feldregeln geprüft. Eine neue Markierung setzt eine zuvor erteilte Bestätigung der Originaltreue zurück.

## Posteingang und Entwürfe

Der Posteingang hält alle übernommenen Rechnungen und ihre Entwürfe dauerhaft bereit. Änderungen werden nach kurzer Eingabepause automatisch gespeichert; der Speicherstand ist sichtbar. Beim nächsten Start wird die zuletzt geöffnete Rechnung einschließlich ihrer Markierungen wiederhergestellt. Vor einem Rechnungswechsel oder dem Schließen sichert die App ausstehende Änderungen; bei einem Speicherfehler bleibt das Fenster geöffnet.

Der Arbeitsbestand liegt unter `%LOCALAPPDATA%\de.erechnung.converter\workspace`. Ältere JSON-Entwürfe aus `Dokumente\E-Rechnung Entwürfe` können über **Entwurf öffnen** zusammen mit ihrer ursprünglichen PDF übernommen werden.

## Vorlagengedächtnis und Erkennungsprofile

**Entwurf speichern** sichert sofort und bestätigt zusätzlich die Ergänzungen für das Vorlagengedächtnis. Das automatische Speichern lernt ausdrücklich nichts.

Manuell ergänzte oder markierte Angaben werden mit ihrer Position und Beschriftung verknüpft. Bei einer gleich aufgebauten Folgerechnung liest Druckwandel den Wert an dieser Stelle automatisch aus. Eine Regel greift nur bei passendem Layout- oder Vorlagenanker; widersprüchliche Treffer ersetzen keinen bereits erkannten Wert, und unsichere Übernahmen sind als prüfbedürftig gekennzeichnet.

Das Vorlagengedächtnis speichert keine konkreten Rechnungswerte, sondern Positionsdaten, Beschriftungen und nicht umkehrbare Absenderkennungen. Unter **Einstellungen → Erkennungsprofile** lassen sich Profile anlegen und umbenennen sowie einzelne gemerkte Zuordnungen anzeigen, deaktivieren und entfernen. Die letzte Bestätigung kann rückgängig gemacht werden. **Gemerkte Ergänzungen löschen** setzt das Profil zurück.

## E-Rechnungen erzeugen

**Für Behörden speichern** (XRechnung, UBL) und **Als PDF-Rechnung speichern** (ZUGFeRD/Factur-X) werden freigegeben, sobald die Pflichtangaben für den gewählten Zweck vollständig und alle Positionen berechenbar sind. Die Leitweg-ID/Käuferreferenz ist nur bei Rechnungen an Behörden erforderlich. Die Ausgaben liegen unter `Dokumente\E-Rechnung Ausgaben`.

Vor jeder fertigen Ausgabe prüft die App die tatsächlich erzeugten Bytes mit den gebündelten Prüfern: KoSIT für XRechnung, Mustang für ZUGFeRD-XML und zusätzlich veraPDF (PDF/A-3b) für PDF-Rechnungen. Schlägt eine Prüfung fehl, entsteht keine fertige Datei.

Die PDF-Rechnung ist eine neue PDF/A-3-Datei mit eingebetteter `factur-x.xml`; die Original-PDF bleibt unverändert. Schriften müssen bereits eingebettet sein. Formulare, Verschlüsselung und widersprüchliche vorhandene E-Rechnungsanhänge werden abgewiesen. Vor der Fertigstellung muss bestätigt werden, dass die Angaben die Originalrechnung korrekt wiedergeben.

## Rechnungsarchiv

Jede fertig gespeicherte E-Rechnung wird automatisch unter `Dokumente\E-Rechnungsarchiv` archiviert, jeweils als PDF mit den maschinenlesbaren XML-Daten. Metadaten und Suchindex liegen in `archiv.sqlite3`; die Rechnungen selbst bleiben normale Dateien in Jahr-/Monatsordnern.

SHA-256-Prüfsummen schützen die Dateien, und jeder Eintrag enthält die Prüfsumme seines Vorgängers. **Archiv prüfen** kontrolliert Datenbank, laufende Nummern, Kette, Dateien und Signaturen und schreibt einen Bericht nach `E-Rechnungsarchiv\Prüfberichte`. Unter **Einstellungen → Archivschutz** können neue Einträge zusätzlich mit einem lokalen Ed25519-Schlüssel bestätigt werden.

> **Wichtig:** Das Archiv erkennt lokale Veränderungen, ersetzt aber weder eine gesetzliche Aufbewahrungsrichtlinie noch unveränderbaren Speicher, externe Zeitstempel oder eine qualifizierte elektronische Signatur.

## Sicherung und Wiederherstellung

Unter **Einstellungen → Sicherung** schreibt die App Archiv, Entwürfe, Original-PDFs, Vorlagengedächtnis, Kanzleiangaben und den Archivschlüssel in eine kennwortgeschützte `.erechnung`-Datei (Kennwort mindestens 12 Zeichen). Eine Kopie auf derselben Festplatte schützt nicht vor einem Plattenausfall.

Die Wiederherstellung prüft die Datei zuerst und zeigt eine Vorschau. Erst nach Bestätigung wird der bisherige Bestand beiseitegelegt und die Sicherung übernommen. Ein unterbrochener Vorgang lässt sich fortsetzen. Bei einem Update legt das Setup zusätzlich einen Snapshot unter `%LOCALAPPDATA%\de.erechnung.converter\update-backup\<Version>` an.

## Diagnose

**Einstellungen → Diagnose** erzeugt einen Bericht mit Versionen und Zählern, aber ohne PDF-, XML-, Bank- oder Rechnungsinhalte und ohne persönliche Dateipfade. Er wird nicht automatisch versendet. Für Fehlermeldungen bitte diesen Bericht statt echter Rechnungen verwenden.

## DATEV-Export

Unter **Einstellungen → Steuerkanzlei** werden Kanzlei- und Kontierungsangaben hinterlegt; der Export selbst ist über das Archiv (**Für die Steuerkanzlei exportieren**) erreichbar. Er erzeugt EXTF-Buchungsstapel und ein Belegpaket für normale EUR-Ausgangsrechnungen mit 7/19 % und Sollversteuerung. Gutschriften, Korrekturen, Abschlags-/Anzahlungs-/Schlussrechnungen und die übrigen Steuerfälle bleiben gesperrt. Der Export ist ein Entwicklungsstand und ersetzt keinen echten Testimport mit der Steuerkanzlei, siehe [datev-export.md](datev-export.md).
