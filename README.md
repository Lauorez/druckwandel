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

## Start auf dem Windows-Vorführrechner

Das Produkt ist eine **Windows-11-Anwendung** (24H2, Build 26100 oder neuer, x64). Nach dem Klonen reicht ein Skript: es prüft bzw. installiert die Build-Werkzeuge, lädt das Validatorenpaket (Windows-JRE), erzeugt die Musterrechnung und baut den NSIS-Installer.

In PowerShell im geklonten Projektordner:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\setup-windows.ps1
```

Das Skript braucht **keine Administratorrechte**. Fehlende Werkzeuge (Node 22, Rust, .NET SDK 10) werden nur für das aktuelle Benutzerkonto nachgeladen. Visual Studio Build Tools und das Windows-SDK werden nicht maschinenweit installiert; `signtool` kommt aus dem NuGet-Paket der Windows SDK BuildTools. Der NSIS-Installer installiert die App für den aktuellen Benutzer. Das Testzertifikat landet in `CurrentUser\TrustedPeople`, nicht in `LocalMachine`.

Ergebnis:

- `artifacts\windows\E-Rechnungs-Assistent-0.3.0-x64-Setup.exe`
- `artifacts\demo\muster-rechnung.pdf`

Nur die Setup-Datei installieren – nicht `npm run desktop` und nicht ein macOS-DMG. Beim Entwicklungsbuild kann SmartScreen bzw. eine Zertifikatsabfrage erscheinen; das lokale Testzertifikat einmalig zulassen. Danach die Musterrechnung in der App öffnen, Angaben prüfen, die Übereinstimmung bestätigen, beide Ausgaben speichern, anschließend Archiv und DATEV. Optional denselben Beleg über den Drucker **E-Rechnung** drucken.

`validators:fetch` muss auf Windows laufen: die gebündelte JRE ist plattformabhängig. Eine auf dem Mac geladene Darwin-JRE darf nicht in den Windows-Installer.

Weitere CLI-Befehle für Entwicklung:

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

Der ZUGFeRD-Export erzeugt eine neue PDF/A-3-Datei aus der geöffneten Rechnung, bettet die berechneten EN-16931-CII-Daten als `factur-x.xml` mit der Beziehung `Alternative` ein und prüft das Gesamtpaket lokal mit Mustang und veraPDF. Die Original-PDF bleibt unverändert. Schriften müssen bereits eingebettet sein; Formulare, Verschlüsselung und widersprüchliche vorhandene E-Rechnungsanhänge werden abgewiesen. Nicht jedes PDF kann umgewandelt werden. Vor der Fertigstellung müssen Sie bestätigen, dass die Angaben die Originalrechnung korrekt wiedergeben.

### Rechnungsarchiv

Jede in der installierten Anwendung fertig gespeicherte E-Rechnung wird automatisch unter `Dokumente\E-Rechnungsarchiv` archiviert. Zu jedem Eintrag liegen eine PDF und die maschinenlesbaren XML-Rechnungsdaten vor: Bei einer Behörden-Datei wird die geöffnete Quell-PDF zusammen mit der XRechnung abgelegt, bei einer PDF-Rechnung die fertige ZUGFeRD-PDF zusammen mit ihren CII-Daten. Metadaten und Suchindex liegen in `archiv.sqlite3`; die eigentlichen Rechnungen bleiben normale Dateien in nach Jahr und Monat gegliederten Ordnern. Gleichnamige Ausgaben werden nicht überschrieben.

SHA-256-Prüfsummen schützen beide Dateien. Jeder Eintrag enthält zusätzlich die Prüfsumme seines Vorgängers und bildet dadurch eine fortlaufende Kette. **Archiv prüfen** kontrolliert SQLite-Datenbank, laufende Nummern, Kettenanschlüsse, PDF/XML-Dateien und vorhandene Signaturen und schreibt einen verständlichen Bericht nach `E-Rechnungsarchiv\Prüfberichte`. Optional können neue Einträge mit einem lokal erzeugten Ed25519-Schlüssel digital bestätigt werden. Der private Schlüssel liegt ausschließlich im lokalen Anwendungsordner; das Aktivieren oder Deaktivieren verändert frühere Einträge nicht.

Die Funktion erkennt lokale Veränderungen, ersetzt aber weder eine gesetzliche Aufbewahrungsrichtlinie noch unveränderbaren Speicher, externe Zeitstempel oder eine qualifizierte elektronische Signatur. Für belastbare Langzeitaufbewahrung müssen Archivordner, SQLite-Datenbank und lokaler Schlüssel regelmäßig gemeinsam gesichert und organisatorische Lösch- und Zugriffsregeln ergänzt werden.

Der vollständige technische Datenfluss und die Sicherheitsgrenzen sind in [docs/architecture.md](docs/architecture.md) beschrieben.

### WP6-Qualitätsgate

    npm run wp6:check

Der Lauf erzeugt das anonymisierte Referenzkorpus, prüft Felder, Positionen, Warnungen und blockierte Sonderfälle und testet zusätzlich 250 reproduzierbare, künstlich erzeugte Rechnungen. Die Reports liegen unter artifacts/corpus-report.json und artifacts/synthetic-fuzz-report.json. Das Gate ist Bestandteil von npm run check und des Desktop-Release-Builds. Seed-Reproduktion, lokale echte Rechnungen und Hinweise zur sicheren Anonymisierung sind in [docs/wp6-corpus.md](docs/wp6-corpus.md) beschrieben.

Die ergänzenden Workspace-, Wiederanlauf- und echten Windows-Drucktests für 0.3.0 sind in [docs/wp7-acceptance.md](docs/wp7-acceptance.md) dokumentiert. Die verbindliche lokale XML-Prüfung vor der Fertigstellung steht in [docs/wp8-acceptance.md](docs/wp8-acceptance.md). Hybrid-PDF/PDF/A steht in [docs/wp9-acceptance.md](docs/wp9-acceptance.md).

## Externe Validierung

Die interne Prüfung bleibt die schnelle Eingabehilfe. Fertige Ausgaben in der nativen App entstehen erst nach unabhängiger KoSIT- (XRechnung) bzw. Mustang-Prüfung (ZUGFeRD/Factur-X-XML) der tatsächlich erzeugten Bytes. PDF-Rechnungen brauchen zusätzlich veraPDF (PDF/A-3b) und einen bytegleichen XML-Extrakt aus der fertigen PDF. Das gebündelte Paket, Versionen und Lizenzen liegen unter `apps/desktop/src-tauri/resources/validators/`. Entwicklungsadapter:

```powershell
$env:KOSIT_VALIDATOR_JAR = 'C:\validator\validator.jar'
$env:KOSIT_SCENARIOS = 'C:\xrechnung-config\scenarios.xml'
npm run validate:external -- test/fixtures/generated/xrechnung-ubl.xml
```

Für formal konforme Hybrid-PDFs gilt: Die Anwendung erzeugt eine neue PDF/A-3-Datei, bettet die geprüfte XML ein und prüft das Ergebnis mit veraPDF. Nicht jedes Eingabe-PDF ist umwandelbar. Die XML-Prüfung von WP8 ersetzt die PDF/A-Prüfung nicht.

Die XML- und Geschäftsregeln des ZUGFeRD-/Factur-X-Profils:

```powershell
$env:MUSTANG_VALIDATOR_JAR = 'C:\validator\Mustang-CLI.jar'
npm run validate:zugferd -- test/fixtures/generated/zugferd-en16931.xml
```

KoSIT und Mustang prüfen unterschiedliche Profile. Eine Factur-X-CII darf deshalb nicht als XRechnung-Ergebnis des KoSIT-Szenarios bewertet werden.

## Funktionsgrenzen dieses Stands

Unterstützt sind normale Rechnungen mit positionsbezogener Umsatzsteuer. Noch nicht enthalten sind Gutschriften, Belegzuschläge/-abschläge, Vorauszahlungen, Rundungsbeträge, mehrere Zahlungswege und komplexe Steuerfälle. Solche Fälle müssen vor produktivem Einsatz ergänzt und mit offiziellen Referenzvalidatoren regressiongetestet werden.

Standardstände:

- XRechnung 3.0.2 / KoSIT-Konfiguration 2026-01-31, Prüfmotor 1.6.3
- ZUGFeRD 2.5.2 / Factur-X 1.09.2 EN16931, Profilkennung `urn:cen.eu:en16931:2017`; XML-Prüfung über Mustang, PDF/A-3b über veraPDF im gebündelten Paket
- UBL 2.1 und UN/CEFACT CII D16B Syntax
