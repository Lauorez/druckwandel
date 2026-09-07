# E-Rechnungs-Assistent

Lokale Desktop-Anwendung zum Übernehmen, Prüfen, Erzeugen und Archivieren elektronischer Rechnungen. Die Verarbeitung arbeitet lokal und sendet keine Rechnungsdaten an externe Dienste.

## Funktionsumfang

- kanonisches, formatunabhängiges Rechnungsmodell
- exakte Dezimalarithmetik und deterministische kaufmännische Rundung
- Steuergruppierung, Summenberechnung und verständliche Geschäftsregelfehler
- ZUGFeRD/Factur-X EN16931 als UN/CEFACT CII
- XRechnung 3.0 als UBL 2.1
- Einbettung von `factur-x.xml` in ein vorhandenes PDF/A-3 mit AFRelationship `Alternative` und Factur-X-XMP-Metadaten
- austauschbare Adapter für KoSIT und veraPDF
- PDF-Textlayer-Extraktion mit Bounding Boxes und Seitenbezug
- Zeilen- und einfache Tabellenspalten-Rekonstruktion
- regelbasierte Feldklassifikation mit Confidence, Quelle und Transformationshistorie
- lokaler OCR-Fallback über eine austauschbare Schnittstelle
- nativer E-Rechnungs-Assistent mit Quellenmarkierung, vollständigen Rechnungsparteien und editierbaren Positionen
- dauerhafter Posteingang mit eigenen Original-PDFs, automatischer Entwurfssicherung und Wiederaufnahme nach Neustart
- deutsche Betragsdarstellung und -eingabe bei kanonischen, exakten Dezimalwerten im Core
- lokales Vorlagengedächtnis für bestätigte Ergänzungen und zuvor nicht erkannte Positionstabellen
- integriertes Rechnungsarchiv mit PDF/XML-Ablage, SQLite-Suche und verketteten SHA-256-Prüfsummen
- optionale digitale Bestätigung neuer Archiveinträge mit einem lokalen Ed25519-Schlüssel
- sichtbare Pflichtfeldvalidierung, atomare Entwürfe sowie XRechnung- und ZUGFeRD-Ausgabe
- nativer Windows-11-Print-Support-Virtual-Printer mit lokaler OXPS-zu-PDF-Konvertierung
- positive und negative Testfälle

## Start

```powershell
npm install
npm run check
npm run generate:fixtures
npm run extract:pdf -- C:\Rechnungen\beispiel.pdf
npm run extract:pdf -- C:\Rechnungen\scan.pdf --ocr
npm run desktop
```

### Gemeinsamer Windows-Installer

Der auslieferbare Windows-Build besteht für Anwender aus genau einer Setup-Datei. Sie enthält die Tauri-Anwendung, den E-Rechnungsdrucker, dessen kleine Windows-Druckbrücke und die benötigte Windows App Runtime:

```powershell
npm run installer:windows
```

Das Ergebnis liegt unter `artifacts/windows/E-Rechnungs-Assistent-<Version>-x64-Setup.exe`; daneben wird eine SHA-256-Prüfsumme erzeugt. Das Setup prüft vor der Installation Windows 11 24H2, Paketidentität und Signaturen, richtet den Drucker für den aktuellen Windows-Benutzer ein und wartet auf seine betriebsbereite Registrierung. Bei einer normalen Deinstallation wird auch der Drucker entfernt. Bei einem Programm-Update bleibt er bestehen und wird nur aktualisiert, wenn das eingebettete Paket neuer ist.

Der aktuelle Entwicklungsbuild enthält ausschließlich den öffentlichen Teil des lokalen Testzertifikats und kann bei der ersten Installation eine Windows-Sicherheitsabfrage auslösen. Für eine Kundenfreigabe müssen Druckerpaket, Anwendung und Setup vertrauenswürdig signiert werden; ein Produktionsbuild lehnt selbstsignierte Druckerpakete und eine unsignierte Setup-Datei ab.

### WP5: virtueller E-Rechnungsdrucker

Die folgenden Einzelbefehle bleiben nur für die Entwicklung und gezielte Druckerdiagnose erhalten. Normale Anwender verwenden ausschließlich den gemeinsamen Windows-Installer. Voraussetzung ist Windows 11 24H2 (Build 26100 oder neuer). Das Entwicklungspaket wird lokal signiert. Build in einer normalen PowerShell:

```powershell
npm run wp5:build
```

Den E-Rechnungs-Assistenten zuerst einmal starten oder einen der mit `npm run desktop:build` erzeugten Installer installieren. Dadurch wird das lokale Protokoll `erechnung-review://` registriert.

Die einmalige Druckerinstallation muss wegen des Entwicklungszertifikats in einer **als Administrator gestarteten PowerShell** erfolgen:

```powershell
# Falls das frühere PoC-Paket aus dem Ordner drucker noch installiert ist:
.\drucker\scripts\uninstall.ps1
npm run wp5:install
```

Danach steht in Windows der Drucker **E-Rechnung** zur Verfügung. Der native Print-Support-Workflow nimmt den Druckdatenstrom entgegen und wandelt OXPS lokal in PDF um. Die paketierte WinUI-Companion übergibt PDF und versionierte Job-Metadaten anschließend atomar an `Dokumente\E-Rechnung Druckeingang`, startet den exakten Job über `erechnung-review://print-job/<UUID>` in der Tauri-App und beendet den Windows-Print-Workflow.

Der Übergabevertrag besteht nach abgeschlossener Verarbeitung aus drei lokalen Dateien. Die Druckbrücke legt zunächst PDF und Metadaten an; erst der Assistent ergänzt die Bestätigung:

```text
<UUID>.pdf             vollständiges Druck-PDF
<UUID>.printjob.json   schemaVersion 1 und Druckjob-Metadaten
<UUID>.review.json     Bestätigung des Assistenten: opened oder failed
```

Für einen kompletten Test:

1. Notepad öffnen und einen kurzen Testtext eingeben.
2. Auf **E-Rechnung** drucken.
3. Prüfen, dass kein Speichern-unter-Dialog erscheint und sich der **E-Rechnungs-Assistent** direkt mit dem gedruckten Dokument öffnet.
4. In `<UUID>.review.json` muss nach erfolgreicher Extraktion `"status": "opened"` stehen.

Entfernen lässt sich das Entwicklungspaket mit `npm run wp5:uninstall`. Für eine Verteilung muss das Entwicklungszertifikat durch ein vertrauenswürdiges Codesigning-Zertifikat beziehungsweise Store-Signing ersetzt werden.

### Fallback-Drucktest ohne WP5

1. `npm run desktop` starten.
2. In einer beliebigen Anwendung **Microsoft Print to PDF** wählen.
3. Als Ziel den in der App angezeigten Ordner `Dokumente\E-Rechnung Druckeingang` wählen.
4. Die App übernimmt die neue PDF beim nächsten Abgleich (etwa alle 1,5 Sekunden) in den Posteingang. Ohne aktive Rechnung öffnet sie den Eingang; andernfalls bleibt die aktuelle Bearbeitung erhalten. Beim allerersten Einrichten vorhandene Dateien werden als ältere Dateien zur ausdrücklichen Übernahme angeboten.

Dieser Weg testet nur den überwachten Druckeingang, falls WP5 auf einem älteren Windows-Build nicht installiert werden kann.

Die E-Rechnungs-API liegt in `src/engine/index.ts`, die PDF-Pipeline in `src/extraction/index.ts`. Beträge und Mengen werden absichtlich als Dezimal-Strings angenommen; JavaScript-`number` ist für Geldwerte nicht Teil des Domain-Modells.

`--ocr` aktiviert den lokalen Tesseract-CLI-Fallback, wenn der Textlayer weniger als 20 Zeichen enthält. Dafür müssen Tesseract und die Sprachdaten `deu`/`eng` lokal installiert und über `PATH` erreichbar sein. Ohne `--ocr` werden gescannte Dokumente sichtbar mit `OCR_REQUIRED` markiert.

### Prüfen und Speichern

Erkannte Werte lassen sich direkt korrigieren. Positionen können über **Hinzufügen** ergänzt und über **Entfernen** gelöscht werden. Beträge erscheinen deutsch formatiert, zum Beispiel 1.234,56; intern bleiben sie kanonische Dezimalstrings.

Seit 0.3.0 hält **Posteingang** alle übernommenen Rechnungen und ihre Entwürfe dauerhaft bereit. Änderungen werden nach kurzer Eingabepause automatisch gespeichert; der Speicherstand ist sichtbar. Beim nächsten Start wird die zuletzt geöffnete Rechnung einschließlich ihrer Markierungen wiederhergestellt. Neue Druckaufträge ersetzen sie nicht. Vor einem Rechnungswechsel oder normalen Schließen sichert die App noch ausstehende Änderungen. Bei einem Speicherfehler bleibt das Fenster geöffnet. Ein harter Abbruch kann noch nicht als gespeichert bestätigte Eingaben verlieren.

Der Arbeitsbestand liegt unter `%LOCALAPPDATA%\de.erechnung.converter\workspace`: `workspace.sqlite3` enthält die Entwürfe, `originals` eigene unveränderte PDF-Kopien. **Entwurf speichern** sichert sofort und bestätigt zusätzlich die Ergänzungen für das Vorlagengedächtnis; das automatische Speichern lernt ausdrücklich nichts. Alte Dateien aus `Dokumente\E-Rechnung Entwürfe` können über **Entwurf öffnen** zusammen mit ihrer ursprünglichen PDF übernommen werden. Die alten Dateien werden nicht gelöscht. Im Browser-Entwicklungsmodus bleibt es beim JSON-Download ohne dauerhaften Posteingang.

**Für Behörden speichern** und **Als PDF-Rechnung speichern** werden jeweils freigegeben, sobald die für den gewählten Zweck erforderlichen Pflichtangaben vollständig und alle Positionen berechenbar sind. Die Leitweg-ID/Käuferreferenz ist nur bei Rechnungen an Behörden erforderlich. Beide fertigen Ausgaben liegen unter Dokumente\E-Rechnung Ausgaben. Im Browser-Entwicklungsmodus werden die Dateien stattdessen heruntergeladen.

Manuell ergänzte oder korrigierte Angaben werden beim Speichern mit ihrer Position und Beschriftung in der geöffneten Rechnung verknüpft. Bei einer ähnlich aufgebauten Folgerechnung desselben Absenders liest der Assistent den neuen Wert an dieser Stelle automatisch aus. Auch eine vollständig übersehene, manuell nachgetragene Positionstabelle kann über ihre Spaltenanordnung gelernt werden. Das Vorlagengedächtnis speichert keine konkreten Rechnungswerte, sondern Positionsdaten, Beschriftungen und nicht umkehrbare Absenderkennungen. Es liegt ausschließlich im lokalen Anwendungsordner und kann in der Oberfläche über **Gemerkte Ergänzungen löschen** zurückgesetzt werden.

Mit **Im PDF markieren** neben einem Feld lässt sich die Quelle gezielt bestimmen: Text anklicken oder einen Rahmen darum ziehen, den markierten Wert prüfen und **Übernehmen** wählen. Mitmarkierte Beschriftungen können vor dem Übernehmen entfernt werden. **Entwurf speichern** oder eine fertige Ausgabe bestätigt die Zuordnung dauerhaft. Die Feldregeln speichern den ausgewählten Textblock statt der ganzen Zeile; benachbarte Bank- und Steuerangaben gelangen dadurch nicht in die Anschrift. Feste Beschriftungen und Spalten dienen zusätzlich als Vorlagenkennung, sodass gleich aufgebaute Rechnungen auch mit einem anderen Absender erkannt werden. Bei mehrfach vorkommenden, gleich plausiblen Werten fordert das Speicherfeedback zur gezielten Markierung auf. Seiten ohne Textlayer lassen sich weiterhin nur manuell erfassen.

Der ZUGFeRD-Export erhält die sichtbaren Seiten des geöffneten PDFs und bettet die berechneten EN-16931-CII-Daten als `factur-x.xml` mit der Beziehung `Alternative` ein. Ein beliebiges Eingabe-PDF wird dadurch nicht automatisch zu einer konformen PDF/A-3-Datei. Die Anwendung weist deshalb nach dem Export ausdrücklich auf die noch ausstehende externe PDF/A-Prüfung hin.

### Rechnungsarchiv

Jede in der installierten Anwendung fertig gespeicherte E-Rechnung wird automatisch unter `Dokumente\E-Rechnungsarchiv` archiviert. Zu jedem Eintrag liegen eine PDF und die maschinenlesbaren XML-Rechnungsdaten vor: Bei einer Behörden-Datei wird die geöffnete Quell-PDF zusammen mit der XRechnung abgelegt, bei einer PDF-Rechnung die fertige ZUGFeRD-PDF zusammen mit ihren CII-Daten. Metadaten und Suchindex liegen in `archiv.sqlite3`; die eigentlichen Rechnungen bleiben normale Dateien in nach Jahr und Monat gegliederten Ordnern. Gleichnamige Ausgaben werden nicht überschrieben.

SHA-256-Prüfsummen schützen beide Dateien. Jeder Eintrag enthält zusätzlich die Prüfsumme seines Vorgängers und bildet dadurch eine fortlaufende Kette. **Archiv prüfen** kontrolliert SQLite-Datenbank, laufende Nummern, Kettenanschlüsse, PDF/XML-Dateien und vorhandene Signaturen und schreibt einen verständlichen Bericht nach `E-Rechnungsarchiv\Prüfberichte`. Optional können neue Einträge mit einem lokal erzeugten Ed25519-Schlüssel digital bestätigt werden. Der private Schlüssel liegt ausschließlich im lokalen Anwendungsordner; das Aktivieren oder Deaktivieren verändert frühere Einträge nicht.

Die Funktion erkennt lokale Veränderungen, ersetzt aber weder eine gesetzliche Aufbewahrungsrichtlinie noch unveränderbaren Speicher, externe Zeitstempel oder eine qualifizierte elektronische Signatur. Für belastbare Langzeitaufbewahrung müssen Archivordner, SQLite-Datenbank und lokaler Schlüssel regelmäßig gemeinsam gesichert und organisatorische Lösch- und Zugriffsregeln ergänzt werden.

Der vollständige technische Datenfluss und die Sicherheitsgrenzen sind in [docs/architecture.md](docs/architecture.md) beschrieben.

### WP6-Qualitätsgate

    npm run wp6:check

Der Lauf erzeugt das anonymisierte Referenzkorpus, prüft Felder, Positionen, Warnungen und blockierte Sonderfälle und testet zusätzlich 250 reproduzierbare, künstlich erzeugte Rechnungen. Die Reports liegen unter artifacts/corpus-report.json und artifacts/synthetic-fuzz-report.json. Das Gate ist Bestandteil von npm run check und des Desktop-Release-Builds. Seed-Reproduktion, lokale echte Rechnungen und Hinweise zur sicheren Anonymisierung sind in [docs/wp6-corpus.md](docs/wp6-corpus.md) beschrieben.

Die ergänzenden Workspace-, Wiederanlauf- und echten Windows-Drucktests für 0.3.0 sind in [docs/wp7-acceptance.md](docs/wp7-acceptance.md) dokumentiert. Der DATEV-EXTF-Export ist der nächste vorgezogene Meilenstein, noch kein Bestandteil dieser Version; Reihenfolge und Umfang stehen in [docs/roadmap-1.0.md](docs/roadmap-1.0.md).

## Externe Validierung

Die interne Prüfung ist schnell und verständlich, ersetzt aber keine offizielle Schema-/Schematron-Prüfung. Für XRechnung wird die KoSIT-Konfiguration 3.0.2 (Release 2026-01-31) unterstützt:

```powershell
$env:KOSIT_VALIDATOR_JAR = 'C:\validator\validator.jar'
$env:KOSIT_SCENARIOS = 'C:\xrechnung-config\scenarios.xml'
npm run validate:external -- test/fixtures/generated/xrechnung-ubl.xml
```

Für formal konforme Hybrid-PDFs gilt: Das Eingabe-PDF muss bereits PDF/A-3 sein. Nach dem Einbetten muss das Ergebnis über `VeraPdfValidator` mit veraPDF geprüft werden. Die Engine und der E-Rechnungs-Assistent behaupten ohne diese unabhängige Prüfung ausdrücklich keine PDF/A-Konformität. Der aktuelle offizielle FeRD-Release ist ZUGFeRD 2.5.2/Factur-X 1.09.2; dessen versionierte Schema- und Schematron-Artefakte sind noch nicht Bestandteil dieses Prototyps.

Die XML- und Geschäftsregeln des ZUGFeRD-/Factur-X-Profils lassen sich separat mit Mustang prüfen:

```powershell
$env:MUSTANG_VALIDATOR_JAR = 'C:\validator\Mustang-CLI.jar'
npm run validate:zugferd -- test/fixtures/generated/zugferd-en16931.xml
```

KoSIT und Mustang prüfen unterschiedliche Profile. Eine Factur-X-CII darf deshalb nicht als XRechnung-Ergebnis des KoSIT-Szenarios bewertet werden.

## Funktionsgrenzen dieses Stands

Unterstützt sind normale Rechnungen mit positionsbezogener Umsatzsteuer. Noch nicht enthalten sind Gutschriften, Belegzuschläge/-abschläge, Vorauszahlungen, Rundungsbeträge, mehrere Zahlungswege und komplexe Steuerfälle. Solche Fälle müssen vor produktivem Einsatz ergänzt und mit offiziellen Referenzvalidatoren regressiongetestet werden.

Standardstände:

- XRechnung 3.0.2 / KoSIT-Konfiguration 2026-01-31
- ZUGFeRD 2.5.2 / Factur-X 1.09.2 EN16931, Profilkennung `urn:cen.eu:en16931:2017`; die passenden FeRD-Prüfartefakte müssen vor einem konformen Produktrelease als versioniertes Standardpaket eingebunden werden
- UBL 2.1 und UN/CEFACT CII D16B Syntax
