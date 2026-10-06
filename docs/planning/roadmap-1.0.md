# Umsetzungsplan bis zur Version 1.0

Stand: 11.09.2026. Ausgangspunkt: Version 0.2.2.

Status: in Umsetzung. WP7 bis WP14 sind implementiert; Abnahme siehe [WP7-Protokoll](../acceptance/wp7-acceptance.md), [WP8-Protokoll](../acceptance/wp8-acceptance.md), [WP9-Protokoll](../acceptance/wp9-acceptance.md), [WP10-Protokoll](../acceptance/wp10-acceptance.md), [WP11-Protokoll](../acceptance/wp11-acceptance.md), [WP12-Protokoll](../acceptance/wp12-acceptance.md), [WP13-Protokoll](../acceptance/wp13-acceptance.md) und [WP14-Protokoll](../acceptance/wp14-acceptance.md). Die Abnahmematrix steht in [acceptance-matrix.md](../acceptance-matrix.md). Keine 1.0- oder fachliche Produktionsfreigabe. Die Nummerierung führt die bisherigen WP1–WP6 fort. Der ursprüngliche MVP-Projektplan bleibt als historische Produktbeschreibung bestehen.

## Ziel und Grenzen

Eine lokal arbeitende Windows-Anwendung, die Rechnungen zuverlässig entgegennimmt, Bearbeitungen erhält, Ausgaben vor der Fertigstellung unabhängig prüft und ihre Daten wiederherstellen kann. Ein lokaler DATEV-Export im EXTF-Format ergänzt die Übergabe freigegebener Ausgangsrechnungen an die Steuerkanzlei. Nicht unterstützte Belege werden verständlich abgewiesen. Eine technische Prüfung ist keine Garantie für die sachliche oder steuerliche Richtigkeit einer Rechnung.

Gesetzt bleiben TypeScript-Fachkern, React/Tauri, Rust für lokale Systemzugriffe und die kleine bestehende Windows-Druckbrücke. Kein Electron, kein zusätzlicher lokaler Webserver, kein dauerhafter Hintergrunddienst und keine neue Rechnungslogik in Rust oder .NET. Windows 11 ab dem bereits vorausgesetzten Build bleibt die erste Zielplattform.

Nicht Bestandteil dieses Plans: macOS-Drucker, Cloud, Peppol, eine eigene Finanzbuchhaltung, DATEV-Onlineanbindung/Belegtransfer, unbeaufsichtigtes Erzeugen allein aufgrund gelernter Zuordnungen und ein vollständiger Scan-/OCR-Workflow. EXTF dient zunächst ausschließlich der dateibasierten Übergabe von Buchungsdaten. Neue kostenpflichtige Komponenten, Veröffentlichungen, Zertifikatsbestellungen und externe Datentransfers benötigen eine gesonderte Entscheidung.

## Geprüfter Ausgangsstand vor WP7 (0.2.2)

- `App.tsx` hält jeweils eine Rechnung im React-Zustand. Neue Druckjobs rufen erneut `openPdfBytes` auf. Eine dauerhafte Bearbeitungswarteschlange fehlt.
- `write_review_draft` schreibt JSON. Eine vollständige Entwurfsverwaltung mit Original-PDF und Wiederaufnahme ist nicht vorhanden.
- Das Vorlagengedächtnis unterstützt Textmarkierungen und bestätigte Korrekturen. Automatisches Zwischenspeichern muss künftig davon getrennt bleiben.
- `src/engine/validators.ts` besitzt Node-/CLI-Adapter; die installierte Tauri-Anwendung führt diese Prüfungen beim Export nicht aus.
- `embedCiiInPdf` erhält die sichtbaren Quellseiten und ergänzt XML/XMP. Es konvertiert die Quelle nicht zu PDF/A-3; selbst die PDF/A-Kennung im XMP ist kein Konformitätsnachweis.
- Das Archiv besitzt SQLite, Datei-Hashes und eine Hash-Kette, aber noch keine integrierte Sicherung/Wiederherstellung. SQLite und Dateisystem sind zusammen keine atomare Transaktion.
- Die Archivmetadaten allein reichen für Buchungsdaten nicht aus: Ein vollständiger fachlicher Rechnungssnapshot, stabile Identität über mehrere Ausgabeformate sowie Konten-/Steuerzuordnungen und eine DATEV-Exporthistorie fehlen bisher.
- Ein gemeinsamer Entwicklungsinstaller und automatisierte Tests existieren. Produktionssignierung, unabhängige Testrechner und Pilotbetrieb sind noch gesonderte Freigabeaufgaben.

## Reihenfolge und Arbeitsweise

Hauptfolge: **WP7 → WP15 (DATEV) → WP8 → WP9 → WP10 → WP11 → WP12 → WP13 → WP14**. Der DATEV-Basisexport hat nach der dauerhaften Vorgangs-/Entwurfsverwaltung die höchste Priorität und erhält einen eigenen testbaren Zwischenstand. WP15 wurde nachträglich ergänzt; die bestehenden Paketnummern bleiben stabil.

WP15 übernimmt die bisher in WP8 geplante Datenbasis für nachgelagerte Exporte: vollständiger archivierter Rechnungssnapshot, formatübergreifende Rechnungsidentität und deren Integritätsbindung. Damit hängt der DATEV-Basisexport nur an WP7, nicht an der vollständigen XML-Prüferintegration, PDF/A-Konvertierung oder neuen Rechnungsfällen. Die DATEV-eigenen Prüfungen und der bestätigte Kanzleikontext bleiben erforderlich. Die frühen Ergebnisse sind ein Test-/Beta-Zwischenstand, keine vorgezogene 1.0-Freigabe.

Der PDF/A-Machbarkeitsnachweis samt Lizenzprüfung steht am Anfang von WP9 und verzögert den DATEV-Basisexport nicht. WP11 enthält eine bewusste Umfangsentscheidung; nicht freigegebene Sonderfälle bleiben auch im DATEV-Export blockiert. Deren spätere Unterstützung braucht ein eigenes DATEV-Mapping und eine eigene Abnahme. Alle übrigen Freigabekriterien bleiben unverändert.

Vor jedem Paket: relevanten Ist-Stand prüfen, vorhandene Nutzerdaten und Änderungen erhalten, Tests für das betroffene Verhalten festlegen. Nach jedem Paket: passende automatisierte Tests, bei Windows-/UI-Änderungen ein nativer Funktionstest und ein kurzer Ergebnisbericht. Installer-Meilensteine nach WP7, WP15, WP9 und für den Release Candidate; keine Versionssprünge für bloße Planung.

## WP7 – Dauerhafter Posteingang und wiederherstellbare Entwürfe

Implementiert in 0.3.0. Die folgenden Punkte bleiben als ursprünglicher Umfang und Abnahmevertrag erhalten; tatsächliche Prüfergebnisse stehen im [WP7-Protokoll](../acceptance/wp7-acceptance.md).

### Umsetzung

1. Einen getrennten, veränderlichen Arbeitsbestand im lokalen Anwendungsordner einführen: `workspace.sqlite3` für Vorgänge/Entwürfe und unveränderte Original-PDFs unter UUID-basierten Pfaden. Die bestehende Archivdatenbank bleibt die Ablage fertiger Ausgaben. Vorhandenes `rusqlite`, UUID und SHA-256 wiederverwenden.
2. Vorgänge über stabile IDs führen: Herkunft/Druckjob-ID, Original-Hash, Bearbeitungsrevision, Extraktionsversion, Entwurf, Quellenzuordnungen, Speicherzeitpunkt und Status. Für übernommene Extraktionsergebnisse einen versionierten Snapshot speichern, damit Token-Zuordnungen nach einem Parserupdate nicht still auf andere Texte zeigen.
3. Alle Eingänge durch denselben Aufnahmeweg führen. Erst eine eigene vollständige PDF-Kopie dauerhaft schreiben, dann den Vorgang verfügbar machen. Wiederholte Zustellung derselben Druckjob-ID ist idempotent; identische PDF-Bytes mit anderer Job-ID werden nicht automatisch als ungewolltes Duplikat verworfen.
4. Deep Links und Ordnerabgleich geordnet verarbeiten. Neue Aufträge ersetzen keinen aktiven Entwurf. Ist die Anwendung leer, darf die erste Rechnung automatisch geöffnet werden; weitere erscheinen im Posteingang. Laufende Analyseergebnisse dürfen nach einem Dokumentwechsel nicht die falsche Rechnung überschreiben.
5. Anwendungsneustart anhand gespeicherter Vorgänge abgleichen. Alte, bislang nicht registrierte Eingangsdateien zunächst als importierbare Altbestände anbieten, nicht ungefragt wieder öffnen. Der UUID-Druckvertrag bleibt erhalten; die bisherige Bestätigung `opened` wird nicht als bloße Empfangsbestätigung umgedeutet.
6. Entwürfe nach kurzer Eingabepause automatisch speichern; Revisionsnummern verhindern überholende Schreibvorgänge. „Wird gespeichert“, „Gespeichert“ und Fehler sichtbar anzeigen. Vor Dokumentwechsel/normalem Schließen ausstehende Speicherung abschließen oder verständlich warnen.
7. Automatisches Sichern bestätigt keine Lernregeln. Bewusstes Speichern/Bestätigen und erfolgreicher Export bleiben die Lernereignisse.
8. Alte JSON-Entwürfe verlustfrei importierbar machen. Weil sie nur einen Quelldateinamen enthalten, bei fehlender oder mehrdeutiger PDF den Benutzer zuordnen lassen. Alte Dateien nicht automatisch löschen.

### Betroffene Stellen

`apps/desktop/src/App.tsx`, `printInbox.ts`, neue eng begrenzte Workspace-UI/-Store-Module, `apps/desktop/src-tauri/src/lib.rs` und neues `workspace.rs`. Bestehende Entwurfsdaten aus `src/review/draft.ts` verwenden. Nur die dabei benötigten Verantwortlichkeiten aus `App.tsx` herauslösen.

### Abnahme

- Zehn Aufträge nacheinander und wiederholte Zustellung derselben Job-ID: kein verlorener oder versehentlich doppelter Vorgang.
- Während Rechnung A bearbeitet wird, trifft B ein: A bleibt unverändert geöffnet und B ist verfügbar.
- Nach bestätigtem Speichern und anschließendem hartem Prozessabbruch ist die letzte bestätigte Revision wiederherstellbar. Noch nicht bestätigte Tastatureingaben werden nicht als garantiert gespeichert dargestellt.
- Tests für Neustart, verspätete Analyse/Schreibantworten, fehlende PDF, defekten Entwurf und verweigerte Schreibrechte.
- Bestehender Kaltstart über den echten Drucker funktioniert weiter, ohne Konsolenfenster.

## WP15 – DATEV-Export als EXTF-Buchungsstapel

### Umfang und fachliche Grenze

Ein lokales Kanzleipaket aus dem Archiv: `EXTF_*.csv` als Buchungsstapel und `Belege.zip` nach der DATEV-XML-Schnittstelle online (document.xml mit GUID, PDF und Rechnungs-XML). Zunächst für bestätigte, fertiggestellte Ausgangsrechnungen des eingerichteten Betriebs in EUR. Die eigene Firma muss ausdrücklich einem Kanzlei-/Mandantenprofil zugeordnet sein; nicht jede geöffnete oder archivierte Rechnung ist automatisch eine Ausgangsrechnung. Zuerst normale Rechnungen mit unterstützter Umsatzsteuer, einschließlich gemischter Steuersätze. Keine Bank-/Zahlungsbuchungen, Kreditoren-/Eingangsrechnungen, Fremdwährungsumrechnung, Stammdatenexporte oder automatische DATEV-Übertragung im ersten Schritt.

EXTF ist ein Buchungsdatenaustausch, kein weiteres E-Rechnungsformat. Konto, Gegenkonto und Buchungsrichtung sind zusätzliche fachliche Angaben und lassen sich nicht zuverlässig aus dem Rechnungs-PDF ableiten. DATEV nennt diese neben Umsatz und Belegdatum als Mussfelder für Bewegungsdaten. [DATEV-Funktionsbeschreibung](https://www.datev.de/content/dam/markenassets/marktplatz/schnittstellen-funktionsumfang/DATEV-Format_CP-Pro.pdf)

### Vorgezogene Datenbasis und Abhängigkeiten

Vor dem EXTF-Adapter innerhalb dieses Pakets den versionierten, vollständigen Snapshot der bestätigten `CalculatedInvoice` archivieren, mit fachlicher Rechnungs-ID und Inhaltsrevision aus WP7. XRechnung und ZUGFeRD derselben bestätigten Revision teilen diese Identität. Snapshot, Originalbezug und erzeugte XML-Bytes durch Hashes binden und ihre inhaltliche Übereinstimmung testen; nicht später aus einem veränderten Arbeitsentwurf exportieren. Neue Archivnachweise versioniert integrieren, alte Ketteneinträge unverändert prüfen. WP8 erweitert diesen Mechanismus später um unabhängige Prüfnachweise, statt eine zweite Datenbasis einzuführen.

Auch das für EXTF benötigte Vorgangsprotokoll für Datei-/SQLite-Abschluss hier umsetzen und auf Wiederanlauf prüfen, nicht erst in WP12. WP12 kann dieses Verfahren für Sicherung und Wiederherstellung wiederverwenden. Kontierungsvalidierung, exakte Betragsabstimmung, DATEV-Formatprüfung und die dokumentierte Testimport-Abnahme bleiben Teil dieses Pakets. Die späteren XML-/PDF/A-Prüfungen werden dadurch nicht vorweggenommen: Der DATEV-Status darf den Konformitätsstatus einer E-Rechnung nicht aufwerten.

### Umsetzung

1. **Formatvertrag festlegen:** Den offiziellen DATEV-Entwicklungsleitfaden für Header und Buchungsstapel, verfügbare Musterdateien und das DATEV-Format-Prüfprogramm beschaffen und mit Version/Checksumme dokumentieren. Formatkennung `EXTF`, konkrete Formatversion, Spaltenreihenfolge/-anzahl, Pflichtfelder, Feldlängen, Datums-/Zahlenregeln, Zeichencodierung, Textquotierung und Zeilenenden anhand dieses Profils implementieren. Keine vermeintlich aktuelle Versionsnummer oder CSV-Voreinstellung aus Blogbeispielen übernehmen. Die Detailversion und Prüfprogramm-Verfügbarkeit sind vor Implementierung noch zu bestätigen. [DATEV Developer Portal](https://developer.datev.de/de/file-format/details/datev-format)
2. **Einmalige Einrichtung:** In einem getrennten Bereich „Angaben der Steuerkanzlei“ Berater-/Mandantennummer, Wirtschaftsjahresbeginn, Sachkontenlänge und ein bestätigtes Konten-/Steuerprofil erfassen. SKR03/SKR04 als Ausgangspunkt anbieten, aber keine universell richtige Kontierung behaupten. Erlöskonten, Debitorenzuordnungen und zulässige Kombinationen aus Automatikkonto/BU-Schlüssel müssen bestätigt und bei fehlender oder widersprüchlicher Zuordnung blockiert werden. Ein Sammeldebitor ist nur eine ausdrücklich freigegebene Alternative. Festschreibungskennzeichen nach vereinbartem Übergabeprozess explizit konfigurieren und im Bericht ausweisen, nicht still festschreiben oder einen universell passenden Default erfinden.
3. **Saubere Modulgrenze:** Einen reinen TypeScript-Adapter unter `src/export/datev/` mit eigenem Buchungsmodell, Mapping, Validierung und Byte-Serialisierung ergänzen. `decimal.js` wiederverwenden. Kontierung bleibt getrennt von Rechnungsmodell und PDF-Vorlagengedächtnis. Rust übernimmt nur sichere Dateiausgabe, Einstellungen und Exportprotokoll; keine parallele Berechnungslogik. Neue Bibliotheken nur bei tatsächlichem Bedarf, etwa für die geforderte Zeichencodierung.
4. **Exakte Buchungssätze:** Aus dem archivierten Rechnungssnapshot Beträge, Datum, Rechnungsnummer, Text und Kontenbezüge ableiten. Gemischte Steuern und unterschiedliche Erlöskonten in passende Teilbuchungen zerlegen. Rundung und Zu-/Abschlagsverteilung müssen zu den archivierten Netto-/Steuer-/Bruttosummen passen. DATEV-Steuerautomatik nicht durch zusätzliche Steuerbuchungen doppelt anwenden. Nicht durch das bestätigte Profil abbildbare Beträge oder Restdifferenzen sichtbar blockieren; keine erfundenen Ausgleichsbuchungen. Betragsvorzeichen und Soll/Haben als explizite, getestete Abbildung behandeln.
5. **Zeiträume und Textgrenzen:** Ein Stapel gehört zu genau einem Mandanten, Wirtschaftsjahr und unterstützten Währungskontext. Jahreswechsel/abweichende Wirtschaftsjahre in eindeutige Stapel aufteilen; das Jahr nicht aus einem verkürzten Datumsfeld erraten. Umlaute, Anführungszeichen und Trennzeichen gemäß Spezifikation serialisieren. Nicht darstellbare Zeichen, zu lange Rechnungsnummern und ungültige Konten melden; keine stillen Kürzungen, Ersatzzeichen oder Änderung der Belegidentität.
6. **Archivoberfläche:** Aktion „Für die Steuerkanzlei exportieren“ mit Zeitraum oder expliziter Rechnungsauswahl anbieten. Vorschau zeigt Rechnungen, Buchungszeilen, Summen und fehlende Zuordnungen. Bereits exportierte Rechnungen standardmäßig ausnehmen und auf Wunsch sichtbar einblenden. Enthält die Auswahl Fehler, den gesamten ausgewählten Export blockieren oder den Benutzer die Auswahl ausdrücklich ändern lassen; keine stillen Teilerfolge. Die UI meldet „Datei erstellt“, niemals ungeprüft „an DATEV übertragen“ oder „gebucht“.
7. **Doppelte Übergaben verhindern:** Exporthistorie mit Stapel-ID, Profil-/Formatversion, Dateihash, Zeitraum, fachlicher Rechnungs-ID/Inhaltsrevision und verwendeten Archivbezügen führen. Dieselbe Rechnung in XML- und Hybrid-PDF-Ausgabe ergibt nicht zwei Buchungen. Auch mehrfach importierte/doppelt gedruckte Belege als mögliche Dubletten markieren; bei widersprüchlichen Daten nicht blind anhand gleicher Rechnungsnummer zusammenführen. Erneutes Herunterladen eines vorhandenen Stapels liefert dieselben gespeicherten Bytes. Bewusster Neu-/Korrekturexport benötigt Bestätigung mit Hinweis auf möglichen Doppelimport in DATEV; ohne Rückkanal ist der dortige Importstatus unbekannt.
8. **Nachvollziehbarkeit ohne Archivmutation:** Erzeugte EXTF-Datei und ein lokales Begleitmanifest unverändert mit Hash aufbewahren. Beziehungen zwischen Stapel und Rechnungen in separaten Exporttabellen führen, vorhandene Rechnungshashes nicht ändern. DATEV-Exportstatus ist kein Rechnungsstatus und EXTF kein zusätzlicher `ArchiveFormat`-Wert. Dateisystem/SQLite-Abschluss mit dem vorgesehenen Vorgangsprotokoll absichern. Bei Altarchiven fehlenden Rechnungssnapshot kontrolliert aus unterstütztem UBL/CII rekonstruieren und prüfen oder Export mit verständlichem Hinweis sperren; nicht aus PDF neu raten oder nur den Archiv-Bruttobetrag verwenden.
9. **Belegpaket mit EXTF:** Jede Ausgabe ist ein Kanzleipaket: EXTF-Buchungsstapel plus `Belege.zip` nach DATEV-XML-Schnittstelle online (document.xml Version 6.0, Belegbilder mit GUID). Dieselbe GUID steht in der EXTF-Spalte Beleglink (`BEDI "<GUID>"`) und in document.xml. Kein lokaler Dateipfad als Beleglink. Die Oberfläche weist auf die Importreihenfolge hin (zuerst ZIP über DATEV Belegtransfer, nicht entpacken, danach CSV). Keine Belegübertragung im Hintergrund und keine Aussage „an DATEV übertragen“.
10. **Sonderfälle gestaffelt freigeben:** Gutschrift-/Korrekturbelege, Steuerbefreiung und Reverse Charge erst nach dem jeweiligen WP11-Teilpaket plus eigenem DATEV-Mapping-/Importtest einschalten. Soll/Haben-Umkehr, Generalumkehr und Belegkorrektur nicht gleichsetzen. Abschlags-/Schlussrechnungen bleiben bis zur gesonderten Freigabe ausgeschlossen. Ein nach E-Rechnungsregeln gültiger Beleg ist nicht automatisch durch das aktuelle DATEV-Profil abbildbar.

### Betroffene Stellen

`src/export/datev/` (neu), innerhalb WP15 vorgezogener archivierter Rechnungssnapshot und Integritätsbindung auf Basis der WP7-Vorgänge, `apps/desktop/src/ArchiveView.tsx`, eigene Export-/Einstellungsansicht und schmale Tauri-Kommandos. Einstellungen/Zuordnungen im veränderlichen Arbeitsbestand, fertige Stapel und deren Beziehungen als gesonderte Exportablage mit SQLite-Metadaten. Profile, Historie und EXTF-Dateien in WP12 sichern; DATEV-Tests in WP13/WP14 aufnehmen.

### Abnahme und externe Voraussetzungen

- Bytegenaue Referenztests für Header/Spalten, Codierung, deutsche Beträge, Datum/Jahreswechsel, Leerfelder, Textquotierung und Feldgrenzen; unabhängiges Wiedereinlesen der erzeugten Datei ergänzen.
- Normale Rechnung, gemischte 7/19-%-Steuern, mehrere Erlöskonten, kleine Rundungsbeträge und bestätigte Kontenprofile gegen unabhängige Soll-Buchungen prüfen. Pro Rechnung stimmen Teilbuchungen mit den archivierten Rechnungsbeträgen überein.
- Fehlende Konten, falscher Mandant, unzulässiges Wirtschaftsjahr, nicht unterstützte Steuerfälle, fremde Währung und unzulässige Zeichen blockieren; keine Erfindung von Konten oder Steuerkennzeichen.
- Mehrfachausgabe als XRechnung/ZUGFeRD, wiederholter Export, abgebrochene Speicherung und Wiederherstellung erzeugen keine unbemerkten zusätzlichen Exportvorgänge. Ein bereits erzeugter Stapel bleibt auch nach Änderung des Kontenprofils bytegleich abrufbar.
- Referenzdateien mit dem offiziellen DATEV-Format-Prüfprogramm prüfen, sobald verfügbar; zusätzlich Import in einen bereitgestellten DATEV-Testbestand mit Prüfung von Konten, Buchungsrichtung, Steuerbeträgen und Belegnummern durch die Steuerkanzlei. Formatprüfung allein ersetzt diese fachliche Abnahme nicht.

Ich kann Adapter, UI, Konfigurationsverwaltung, Historie und automatisierte Tests selbst implementieren. Ein bestätigtes Kanzleiprofil und ein echter DATEV-Testimport brauchen die Kanzlei beziehungsweise einen bereitgestellten Testzugang. Falls das Portal für Prüfarbeitsmittel eine Anmeldung verlangt, Zugang/Dateien bereitstellen lassen; keine Registrierung oder Übermittlung echter Belege ohne Auftrag. Ohne diesen Importtest den Export als noch nicht abschließend abgenommen kennzeichnen, nicht als DATEV-zertifiziert bewerben.

## WP8 – Verbindliche lokale XML-Prüfung und kontrollierte Fertigstellung

### Umsetzung

1. Ein gemeinsames Ergebnisformat für Prüfung, Fehler, Warnungen, Regelversion und Artefakt-Hashes definieren. Rechenprüfung und XML-Erzeugung bleiben im TypeScript-Kern.
2. KoSIT mit passender XRechnung-Konfiguration sowie einen passenden ZUGFeRD-/Factur-X-Prüfsatz versioniert bündeln. Abhängigkeiten, Checksummen und Lizenzen in einem Standardpaket-Manifest festhalten; Versionen beim Umsetzungstermin gegen die Herausgeber prüfen.
3. Rust startet ausschließlich gebündelte Prüfprogramme mit festen Argumenten, Zeit-/Ausgabegrenzen und ohne Fenster. Eine passende Java-Laufzeit bei Bedarf privat mitliefern, nicht beim Kunden über PATH oder eine Java-Installation voraussetzen. Kein HTTP-Daemon und kein zusätzliches Node-Runtime-Paket für die bestehenden Entwicklungsadapter. KoSIT unterstützt den lokalen CLI-Betrieb. [KoSIT-Dokumentation](https://github.com/itplr-kosit/validator)
4. Maschinenlesbare Prüfberichte auswerten. Exitcode 0 allein genügt nicht. Fehlender/unlesbarer Bericht, Zeitüberschreitung und unbekannter Status gelten als nicht erfolgreich; insbesondere den bisherigen KoSIT-Adapter dagegen härten. Externe XML-/Netzwerkauflösung im Prüflauf unterbinden.
5. Ausgabe zunächst intern vorbereiten, exakt diese Bytes prüfen und erst danach zur fertigen Ausgabe/Archivierung freigeben. Ein kurzlebiger nativer Prüfbeleg bindet das Ergebnis an die Artefakt-Hashes und die Entwurfsrevision; spätere Änderungen machen ihn ungültig.
6. Fehler deutschen Feldern zuordnen, zur betroffenen Stelle führen und Fortschritt/Abbruch anbieten. Im normalen Kundenablauf keine ungeprüfte Datei als fertige E-Rechnung ausgeben; Entwürfe bleiben speicherbar.
7. Fertigstellung mit stabiler Vorgangs-ID und Exportrevision idempotent machen. Wiederholung nach verlorener Antwort darf nicht versehentlich einen zweiten Archiveintrag erzeugen. Absichtlicher neuer Export bleibt möglich.
8. Prüfbericht, Originalbezug und Regelversion in das Archiv aufnehmen. Neue Hash-Einträge versionieren und den Bericht-Hash einschließen; alte Ketteneinträge unverändert nach dem alten Verfahren prüfen. Historische Ausgaben ohne damaligen Bericht nicht rückwirkend als geprüft ausgeben.
9. Den bereits in WP15 eingeführten Rechnungssnapshot und dessen fachliche Identität wiederverwenden. Den unabhängigen Prüfbeleg zusätzlich an Snapshot, Inhaltsrevision und tatsächlich ausgegebene XML-/PDF-Bytes binden. Bestehende DATEV-Stapel und ältere Rechnungsrevisionen unverändert lassen; frühere Ausgaben ohne unabhängige Prüfung nicht nachträglich als damals geprüft kennzeichnen.

### Betroffene Stellen und Abnahme

`src/engine/validators.ts`, `src/engine/`, `apps/desktop/src/archiveStore.ts`, Exportaktionen, native Prüfadapter und `archive.rs`, Installer-Ressourcen.

Positive und absichtlich ungültige XML-Fixtures gegen die echten Prüfer testen. Zusätzlich Timeout, fehlenden Prüfer, kaputten Bericht, Änderung während der Prüfung und doppelte Fertigstellung testen. Ein Prüf- oder Archivfehler darf keinen Erfolg melden. Der gesamte Ablauf muss ohne Internet und ohne systemweit installiertes Java funktionieren.

## WP9 – Konforme Hybrid-PDFs und konsistente Rechnungsinhalte

### Früher Machbarkeitsnachweis

Eine begrenzte Auswahl lokaler PDF/A-Konverter gegen vorhandene Print-PDFs und problematische Fixtures evaluieren: eingebettete/fehlende Schriften, Unicode, Transparenz, mehrere Seiten, Formulare, Anhänge und beschädigte Dateien. Bild, Textinhalt und Seitenzahl vor/nach Konvertierung vergleichen, anschließend unabhängig validieren. Nicht nur passende Metadaten setzen. veraPDF stellt eine unabhängige PDF/A-Prüfung bereit. [veraPDF-Dokumentation](https://docs.verapdf.org/validation/)

Vor Integration eine kurze Entscheidung mit Qualität, Laufzeit, Paketgröße und Lizenzbedingungen dokumentieren. Ghostscript wäre beispielsweise nur ein Kandidat, keine bereits beschlossene Abhängigkeit; dessen Anbieter verweist für die Lizenzierung auf Artifex. [Lizenzierungsseite](https://ghostscript.com/licensing/) Keine kommerzielle Lizenz bestellen oder unklare Weiterverteilungsbedingungen still akzeptieren. Ist kein tragfähiger Konverter verfügbar, bleibt dieses Freigabekriterium offen; XML-Ausgabe und andere Pakete können trotzdem weiterentwickelt werden.

### Umsetzung nach positivem Nachweis

1. Original unverändert behalten; eine neue PDF/A-3-Ausgabe erzeugen, dieselben bereits erzeugten CII-Bytes einbetten und das endgültige Gesamtpaket erneut prüfen. Nach erfolgreicher Prüfung keine Metadaten oder Bytes mehr verändern.
2. `embedCiiInPdf` für vollständige Metadaten, vorhandene Anhänge und bereits enthaltene Rechnungs-XML härten. Unbekannte oder widersprüchliche bestehende E-Rechnungsanhänge nicht still überschreiben oder vervielfachen.
3. Summen und belegbare Angaben mit der Quellrechnung vergleichen. Abweichungen sichtbar machen. Automatische Vergleiche helfen bei der Prüfung, können aber nicht sämtliche inhaltlichen Widersprüche beliebiger PDFs beweisen.
4. Für 1.0 einen Konverter-Workflow beibehalten: Der Benutzer bestätigt, dass ergänzte Werte die Originalrechnung korrekt wiedergeben. Tatsächliche Änderungen am Rechnungsinhalt verlangen eine korrigierte Quelle aus dem Ursprungsprogramm. Kein stilles Neurendern und keine automatische Änderung einer bereits ausgestellten Rechnung. Ein eigener Rechnungsgenerator wäre ein separates Produktfeature.
5. Die Prüfansicht muss die tatsächlichen Ausgabedaten verständlich zeigen; Belege mit nicht aufgelösten Widersprüchen nicht fertigstellen. Herkunft, Bestätigung und Quell-Hash mit archivieren.

### Abnahme

Die freigegebene PDF-Testmenge besteht XML-/Profil- und PDF/A-Prüfung. Aus der fertigen PDF extrahierte XML ist bytegleich zur geprüften und separat archivierten XML. Negativtests blockieren nicht konvertierbare PDFs, widersprüchliche Anhänge und unaufgelöste Abweichungen. Es gibt keine Zusage, jedes beliebige PDF konvertieren zu können.

## WP10 – Messbare Erkennungsqualität und kontrollierbares Lernen

Implementiert; Abnahme siehe [WP10-Protokoll](../acceptance/wp10-acceptance.md). Die folgenden Punkte bleiben als ursprünglicher Umfang erhalten.

1. Bestehendes Korpus erweitern: mehrere unabhängig erzeugte Layoutfamilien, Seitenumbrüche, ähnliche Absender-/Empfängerblöcke, wechselnde Fußzeilen, mehrfach vorkommende Werte und veränderte Vorlagen. Vorhandene echte Belege nur lokal verwenden; zusätzliche Kundenbelege müssen bereitgestellt werden.
2. Lern- und Prüfrechnungen trennen. Regeln auf Rechnung A bestätigen und auf unbekannten B/C prüfen, einschließlich absichtlich ähnlicher, aber unpassender Vorlagen. Neue Werte dürfen niemals durch alte Rechnungswerte ersetzt werden.
3. Quellen und Unsicherheiten durch den vollständigen Ablauf erhalten. Bei widersprüchlichen Regeln oder schwacher Übereinstimmung keine stillen Ersetzungen. Layoutanker bei Verschiebungen auswerten; absolute Regionen nur bei ausreichender Übereinstimmung anwenden.
4. Einzelne gemerkte Zuordnungen verständlich anzeigen, deaktivieren und entfernen können; eine versehentliche Bestätigung rückgängig machen. Bestehende Regeln kompatibel migrieren und ihre Herkunft nachvollziehbar halten.
5. Einen lokalen Qualitätsbericht mit Feld-/Positionsgenauigkeit, Fehlzuordnungen, Blockierungen und nötigen Korrekturen erzeugen. Keine Telemetrie oder Zusendung von Rechnungswerten im Hintergrund.

Betroffene Stellen: `src/extraction/`, `src/learning/correction-memory.ts`, Quellenmarkierung, Einstellungen und bestehende Corpus-/Lerntests. Abnahme: keine Regression im vorhandenen Referenzbestand; alle neuen Gegenbeispiele bestehen. Synthetische Erfolge ausdrücklich nicht als gemessene Kundenquote darstellen.

## WP11 – Rechnungsumfang gezielt erweitern

**WP11a**, **WP11b** und **WP11c** sind implementiert; Abnahme siehe [WP11-Protokoll](../acceptance/wp11-acceptance.md).

- **WP11a:** Gutschrift-/Rechnungskorrektur-Dokumente mit explizitem Dokumenttyp und Bezug zur Ursprungsrechnung sowie positions-/belegbezogene Zu- und Abschläge. Steuergruppen, Rundung und Betragssummen dabei vollständig modellieren; „Rabatt“ als Wort allein darf keine falsche Blockierung auslösen.
- **WP11b:** Klar abgegrenzte steuerfreie Fälle und Reverse Charge mit ausdrücklicher Benutzerangabe und den jeweiligen Pflichtinformationen. Kategorien nicht aus „0 %“ erraten. Vor Umsetzung fachliche Fallmatrix anhand der offiziellen Regeln festlegen.
- **WP11c, separat zu entscheiden:** Anzahlungen, Abschlags- und Schlussrechnungen mit bezahlten Beträgen und Belegbezügen. Nicht als bloßes zusätzliches Dropdown implementieren.

Je freigegebenem Teilpaket: `src/domain/`, Berechnung, Review-Draft, UBL/CII, Extraktion und `unsupported-cases.ts` gemeinsam erweitern, alte Entwürfe migrieren und positive/negative Referenzfälle durch WP8/WP9 schicken. Die Ausgabe wird erst freigeschaltet, wenn alle Schichten den Fall unterstützen. Bei einer engeren 1.0 bleiben nicht freigegebene Teilpakete ausdrücklich blockiert und in der Produktbeschreibung ausgeschlossen.

Ich kann Modellierung, Implementierung und technische Referenztests ausführen. Welche Fälle für die ersten Betriebe unverzichtbar sind und ob die fachliche Behandlung im realen Geschäftsfall passt, benötigt Nutzer-/Fachfeedback. Bis dahin gibt es keine Behauptung einer uneingeschränkten Handwerkstauglichkeit.

## WP12 – Sicherung, Wiederherstellung und Absturzbehandlung

Implementiert. Die folgenden Punkte bleiben als ursprünglicher Umfang erhalten; Prüfergebnisse stehen im [WP12-Protokoll](../acceptance/wp12-acceptance.md).

1. Eine Anwendungssicherung für Archiv, Arbeitsbestand/Originale, Entwürfe, Vorlagengedächtnis, Einstellungen, DATEV-Profile/-Zuordnungen, Exporthistorie/-dateien und Schlüssel anbieten. Formatversion, Dateiliste und Prüfsummen in ein Manifest aufnehmen. Private Schlüssel niemals ungeschützt in eine portable Sicherung legen; passwortgeschützte, authentifizierte Verschlüsselung mit einer gepflegten Bibliothek verwenden, keine eigene Kryptografie.
2. Konsistente SQLite-Snapshots über die Backup-API erstellen. Für den gemeinsamen Snapshot beider Datenbanken und veränderlicher Dateien Schreiboperationen kurz koordiniert pausieren; unveränderliche PDF/XML-Dateien anschließend anhand des Snapshots kopieren. Nicht einfach eine laufende WAL-Datenbankdatei kopieren. [SQLite Backup API](https://www.sqlite.org/backup.html)
3. Lokalen Signierschlüssel unter Windows über DPAPI im Benutzerkontext schützen und bestehende Schlüssel vorsichtig migrieren. Für Rechnerwechsel separat verschlüsselten Schlüsseltransport vorsehen: Windows-Schutz allein ist normalerweise an Benutzer/Rechner gebunden. [Microsoft DPAPI](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
4. Wiederherstellung zunächst in einen isolierten Bereich schreiben; Passwort, Grenzen/Pfade, Schema, Dateien und Archivkette prüfen. Erst nach Vorschau und Bestätigung aktivieren, den bisherigen Datenbestand dabei wiederherstellbar erhalten. Zwei unabhängige Archivketten in 1.0 nicht automatisch zusammenführen.
5. Datei-/Datenbankoperationen beim Export und bei Wiederherstellung über ein kleines dauerhaftes Vorgangsprotokoll absichern. Nach Abbruch eindeutig fortsetzen oder sichtbar zur Reparatur anbieten; verdächtige Dateien nicht automatisch löschen. Hash-Ketten nicht passend zu veränderten Dateien neu berechnen.
6. Sicherungserinnerung und frei wählbares Ziel ergänzen. Eine Kopie auf derselben Platte nicht als Schutz gegen einen Plattenausfall darstellen.

Betroffene Stellen: neues natives Sicherungsmodul, `archive.rs`, Workspace-Speicher, Einstellungen. Abnahme: Wiederherstellung in ein leeres Testprofil mit bytegleichen Belegen, identischer Archivkette und fortsetzbaren Entwürfen; falsches Passwort, manipuliertes Paket, Pfadtraversal, fehlender Schlüssel, volle Platte und Prozessabbruch. Ein echter Rechnerwechsel wird zusätzlich auf einem zweiten System geprüft, sobald vorhanden.

## WP13 – Wartbare und sichere Auslieferung

Implementiert. Die folgenden Punkte bleiben als ursprünglicher Umfang erhalten; Prüfergebnisse stehen im [WP13-Protokoll](../acceptance/wp13-acceptance.md).

1. Ein vollständiges Release-Gate für TypeScript, UI, Corpus, Rust, .NET, echte Validatoren, DATEV-Referenz-/Formatprüfungen und Installer zusammenführen. Bestehende Skripte wiederverwenden; Quelltext-Stringtests des Installers durch echte Installations-/Update-Szenarien ergänzen. Nicht verfügbare offizielle DATEV-Prüfungen sichtbar als ausstehend behandeln, nicht als bestanden. CI-Konfiguration vorbereiten; Hosting/Runner nur mit vorhandenem Zugang anbinden.
2. Herkunft und Version aller gebündelten Komponenten, Lizenzhinweise und eine Komponentenliste dokumentieren. Abhängigkeiten auf bekannte Schwachstellen prüfen; blockierende Befunde beheben oder die betroffene Funktion nicht ausliefern.
3. Native Eingabegrenzen prüfen: Deep Links, Dateien, JSON, XML, Archiv-/Backup-Pfade, Symlinks/Junctions, Größenbegrenzung und Prozessargumente. Lang laufende Prüfungen abbrechbar halten, kein Konsolenfenster und keine Rechnungsdaten in allgemeinen Fehlerlogs.
4. Signierpipeline für Anwendung, Helfer, Druckerpaket und Installer vorbereiten. Release-Prüfung darf keine Testzertifikate, fehlenden Signaturen oder ausgelassenen Qualitätsgates akzeptieren. Produktionszertifikat/Signierdienst, Herausgeberidentität und gegebenenfalls Paketidentitätsmigration sind externe Voraussetzungen; vorhandene Installationen dabei berücksichtigen.
5. Updates über den bestehenden gemeinsamen Installer absichern: Entwurfs-/Archivmigration, Druckererhalt und Sicherung vor Migration. Bei fehlgeschlagenem Update einen dokumentierten Wiederherstellungsweg bieten; keine ungeprüfte Abwärtsmigration. Online-Autoupdate ist für 1.0 kein Muss und erfordert später eine vertrauenswürdige Veröffentlichungsstelle.
6. Einen vom Nutzer bewusst exportierten Diagnosebericht anbieten: Versionen, Fehlercodes und Komponentenstatus, standardmäßig keine PDF/XML-Inhalte, Bankdaten oder personenbezogenen Dateipfade. Vorschau vor Weitergabe; kein automatischer Versand.

Abnahme: Neuinstallation, Update vom gesicherten 0.2.2-Teststand, Deinstallation ohne Verlust von Nutzerdaten sowie anschließende Neuinstallation. Druckertests auf bereitgestellten unterstützten Windows-Systemen mit normalem Benutzerkonto; Rechtebedarf verständlich behandeln. Isolierte Tests verwenden, nicht wiederholt den echten Nutzerbestand zurücksetzen.

## WP14 – Release Candidate und dokumentierte Freigabe

Implementiert als lokaler Release Candidate, **ohne 1.0-Kundenfreigabe**. Prüfergebnisse: [WP14-Protokoll](../acceptance/wp14-acceptance.md), [Abnahmematrix](../acceptance-matrix.md), [Pilotunterlage](../pilot-guide.md).

1. Vollständige Abläufe automatisiert und im echten Tauri-Fenster prüfen: Drucken bei geschlossener App, Warteschlange, Bearbeiten/Markieren/Lernen, Neustart, beide E-Rechnungsexporte, Archivsuche/-prüfung, DATEV-Stapelexport inklusive Dublettenschutz, Sicherung und Wiederherstellung.
2. Bedienbarkeit prüfen: Tastatur, Fokus, verständliche Fehler, Lade-/Speicherfeedback, kleine Fenster sowie 100/150/200-%-Skalierung. Wiederkehrende Schritte dürfen keine Entwicklerkenntnisse voraussetzen.
3. Eine Abnahmematrix mit Datum, App-/Standardversion, Betriebssystem, Testdaten-ID und Ergebnis pflegen. Nicht verfügbare Kombinationen als ungeprüft markieren, nicht aus einem lokalen Test ableiten.
4. Pilotunterlagen vorbereiten: kurze Anleitung, unterstützte Rechnungsfälle, Grenzen, Wiederherstellung und datensparsamer Fehlerbericht. Rückmeldungen als reproduzierbare Regressionstests aufnehmen.

Technischer Abschluss: Alle vereinbarten Pakete bestanden; keine bekannten kritischen Datenverlust-/Falschausgabeprobleme; negative Fälle blockieren; endgültige Ausgaben unabhängig geprüft; Wiederherstellung nachgewiesen; vollständiges Release-Gate erfolgreich.

Kundenfreigabe 1.0 zusätzlich: Produktionssignaturen vorhanden, vereinbarter Rechnungsumfang bestätigt, benötigte Zielsystemtests einschließlich DATEV-Testimport für den beworbenen EXTF-Umfang durchgeführt und Pilotfeedback ausgewertet. Lokale Tests ersetzen keine Pilotbetriebe oder unabhängige fachliche/Sicherheitsprüfung.

## Was ich selbst erledigen kann – und was offen bleibt

| Bereich | Selbst umsetzbarer Anteil | Externe Voraussetzung/Grenze |
| --- | --- | --- |
| Posteingang, Entwürfe, Lernen | Implementierung, Migration, lokale und automatisierte Tests | Zusätzliche echte Belege für breitere Aussagekraft |
| Formatprüfung | Adapter, gebündelte Laufzeit/Regeln, Fehlerbehandlung, Prüfnachweise | Passende Weiterverteilungsrechte der konkret ausgewählten Pakete |
| PDF/A | Evaluation, Integration und Referenztests | Lizenz-/Beschaffungsentscheidung, falls kein geeigneter frei weiterverteilbarer Weg besteht |
| Rechnungsfälle | Technische Modellierung, UI, Export und Regeltests | Gewünschter Produktumfang und fachliche Bewertung realer Sonderfälle |
| DATEV EXTF | Adapter, Konfiguration, Archiv-UI, Dublettenschutz, Exporthistorie und Referenztests | Bestätigtes Kanzlei-/Kontenprofil, offizielle Prüfarbeitsmittel und echter DATEV-Testimport |
| Sicherung/Sicherheit | Lokale Umsetzung, Negativtests und Testprofil-Wiederherstellung | Zweiter Rechner/weitere Konten und unabhängiger Audit für zusätzliche Absicherung |
| Installer/Updates | Build-, Signier- und Migrationspipeline, lokale Testpakete | Produktionsidentität, Zertifikat/Signierdienst; Veröffentlichungsziel bei Online-Updates |
| Pilotbetrieb | Anleitung, Testmatrix, Diagnosewerkzeuge, Fehlerbehebung | Betriebe gewinnen, Einverständnisse und tatsächliche Nutzung/Rückmeldungen |

## Erster ausführbarer Auftrag

Der Einstieg **WP7**, der DATEV-Basisexport **WP15**, die verbindliche lokale XML-Prüfung **WP8**, die Hybrid-PDF/PDF/A-Prüfung **WP9**, die Erkennungsqualität **WP10**, **WP11** (Gutschrift/Korrektur, Steuerfälle, Abschlags-/Schlussrechnungen), **WP12** (Sicherung/Wiederherstellung), **WP13** (Auslieferung) und **WP14** (Release Candidate) sind umgesetzt. Die **Kundenfreigabe 1.0** bleibt offen: Produktionssignaturen, DATEV-Testimport, Pilotfeedback.
