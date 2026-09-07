# Technische Architektur

Das System trennt Druckannahme, Arbeitsbestand, PDF-Analyse, Rechnungsmodell, Ausgabe, Vorlagengedächtnis und Archiv. Rechnungsdaten verlassen den Rechner in keiner dieser Schichten.

## 0. Installation und Komponentenlebenszyklus

Unter Windows wird nach außen genau eine NSIS-Setup-Datei verteilt. Sie enthält die Tauri-Anwendung sowie das signierte MSIX des Druckers einschließlich dessen Windows-App-Runtime-Abhängigkeit. Ein Tauri-Installer-Hook prüft mindestens Windows Build 26100, Paketidentität, Herausgeber und Signaturen, bevor Dateien der Hauptanwendung installiert werden. Danach registriert er das MSIX für den angemeldeten Windows-Benutzer, wartet auf die Queue `E-Rechnung` und stellt den benutzerbezogenen Print-Workflow-Dienst sicher.

Die Komponenten bleiben intern absichtlich getrennt: Tauri besitzt den normalen Anwendungslebenszyklus, während Windows die Print-Support-Erweiterungen anhand ihrer MSIX-Paketidentität aktiviert. Beim Anwendungsupdate ruft Tauri den alten Uninstaller mit dem Updatekennzeichen auf; der Drucker bleibt dabei registriert. Nur eine echte Deinstallation entfernt auch das Druckerpaket. Die gemeinsam erzeugte Setup-Datei ist damit eine Distributionsgrenze, keine Auflösung der Laufzeitgrenzen.

Das Buildskript prüft die einzubettenden Pakete erneut und erzeugt eine SHA-256-Prüfsumme des fertigen Setups. Im Entwicklungsmodus darf das öffentliche Testzertifikat mitgeführt werden; der private PFX-Schlüssel wird nie eingebettet. Der Produktionsmodus akzeptiert weder ein selbstsigniertes Druckerpaket noch ein unsigniertes Setup.

## 1. Druckannahme und Windows-Druckbrücke

Unter Windows registriert das MSIX-Projekt in `drucker/` einen Print-Support-Virtual-Printer namens **E-Rechnung**. Windows übergibt den Druckjob als OXPS an den Background Task. Dieser erzeugt lokal ein PDF. Die als WinUI-App paketierte Druckbrücke kopiert anschließend PDF und Metadaten atomar nach `Dokumente\E-Rechnung Druckeingang`:

    <UUID>.pdf
    <UUID>.printjob.json

PDF und Metadaten werden erst unter temporären Namen geschrieben und nach erfolgreichem Flush umbenannt. Danach öffnet die Druckbrücke `erechnung-review://print-job/<UUID>`. Der Deep Link enthält absichtlich nur eine UUID, niemals einen frei wählbaren Dateipfad. Nach erfolgreichem Öffnen ergänzt die Tauri-App `<UUID>.review.json` als Bestätigung.

Die WinUI-Komponente bleibt bewusst im System: `windows.printSupportJobUI` ist der von Windows bereitgestellte Vordergrundvertrag, den `PrintWorkflowUILauncher` aus dem isolierten Druck-Background-Task aktiviert. Sie enthält keine Rechnungslogik und ist kein zweites Produkt-Frontend. Normalerweise übergibt sie den Job automatisch an Tauri und beendet sich; ihr sichtbares Fenster ist nur Fehler- und Wiederholungsfallback. Eine direkte Kopplung des Background-Tasks an Tauri würde die Windows-Aktivierungs- und Paketgrenze umgehen und den bereits stabilen Druckpfad wieder fragil machen.

Die Oberfläche übernimmt Start-Links über die offiziellen Tauri-Kanäle `getCurrent()` und `onOpenUrl()`. Die Tauri-Rust-Schicht validiert danach UUID, Metadaten, Dateiendung, Größenlimit und den kanonischen Pfad innerhalb des Druckeingangs. Nach erfolgreichem Öffnen schreibt die App das Acknowledge `<UUID>.review.json`.

## 1a. Dauerhafter Arbeitsbestand (seit 0.3.0)

Der veränderliche Arbeitsbestand ist bewusst vom Archiv fertiger Ausgaben getrennt. `workspace.rs` verwendet die vorhandenen Bibliotheken `rusqlite`, UUID und SHA-256. Unter `%LOCALAPPDATA%\de.erechnung.converter\workspace` liegen `workspace.sqlite3` und `originals/<UUID>.pdf`. Die Datenbank arbeitet mit WAL, `synchronous=FULL`, Schema-Version und Revisionsvergleich. Ein Vorgang besitzt Identität, Herkunft, Original-Hash, Status, Speicherrevision und optional einen vollständigen Bearbeitungssnapshot; die Listen sind auf 100 Treffer pro Seite begrenzt.

Die Aufnahme schreibt zuerst eine eigene PDF-Datei mit exklusivem Anlegen und `sync_all`, danach wird der Datenbankeintrag bestätigt. Ein kontrollierter Fehler entfernt nur die für diesen Versuch angelegte Datei. Nach einem harten Abbruch zwischen Dateischreiben und Datenbankabschluss kann eine nicht referenzierte Originaldatei übrig bleiben; SQLite und Dateisystem sind keine gemeinsame Transaktion. Beim Öffnen werden UUID, kanonischer Pfad und PDF-Hash erneut geprüft. Ein fehlendes oder verändertes Original wird nicht still ersetzt. Der Arbeitsbestand ist damit absturzrobuster, aber weder ein Backup noch ein manipulationssicheres Archiv.

Eine Druckjob-ID ist ein eindeutiger Empfangsschlüssel: Wiederholung derselben ID legt keinen zweiten Vorgang an, andere Bytes unter dieser ID werden abgewiesen. Identische PDFs mit unterschiedlichen Job-IDs bleiben getrennte Eingänge. Für lose Dateien im Druckeingang besteht der Schlüssel aus kanonischem Pfad-Hash, Änderungszeit und Größe. Der erste Abgleich merkt bereits vorhandene Dateien dauerhaft als Altbestand. Nach dieser Einrichtung neu eintreffende Dateien werden auch dann aufgenommen, wenn die App zwischenzeitlich geschlossen war. Erst tatsächliches Öffnen und Laden bestätigt einen Druckjob als `opened`; bloßes Einreihen tut das nicht.

`useWorkspace.ts` ordnet Aufnahme, Aktivierung und Deep Links über eine gemeinsame Warteschlange. Ein neuer Eingang unterbricht keine aktive Rechnung. `workspaceStore.ts` enthält den versionierten `WorkspaceSnapshot` und einen `DraftWriter` pro aktivem Vorgang. Der Snapshot speichert unveränderte und durch Lernen ergänzte Extraktion, Ausgangsentwurf, aktuellen Entwurf, Markierungen, noch unbestätigte Quellen und Fertigstatus. Die Version `text-layout-v1` bindet die gespeicherten Token-Zuordnungen an das gespeicherte Analyseergebnis. Wiederaufnahme extrahiert nicht neu; unbekannte Versionen und beschädigte Snapshots werden sichtbar abgewiesen.

Nach 450 ms Eingabepause schreibt der `DraftWriter` sequentiell mit erwarteter Revision. Kommen während eines Schreibens weitere Änderungen, schreibt er sie anschließend, bevor `flush()` Erfolg meldet. Die Oberfläche zeigt „Wird gespeichert“, „Gespeichert“ oder einen Fehler mit Wiederholung an. Dokumentwechsel und normales Fensterschließen warten auf diese Sicherung; bei Speicherfehlern bleibt die aktive Bearbeitung erhalten. Während eines laufenden Imports/Exports wird das Schließen mit einem Wartehinweis verhindert. Nicht bestätigte Eingaben vor hartem Prozessabbruch oder Stromverlust sind nicht garantiert. Auch unfertige Zahleneingaben bleiben im Entwurf erhalten und dürfen die fachliche Exportprüfung nicht umgehen.

Alte JSON-Entwürfe enthalten keinen belastbaren PDF-Bezug. Deshalb verlangt die Übernahme ausdrücklich die zugehörige Original-PDF, erhält alle eingegebenen Werte und verwirft alte Token-Verweise, statt sie auf neu extrahierten Text zu übertragen. Quelldateien werden nicht gelöscht. Automatische Speicherung ist kein Lernereignis; erst bewusstes Speichern oder erfolgreicher Export bestätigt Ergänzungen.

## 2. PDF-Analyse

PDF.js liest den vorhandenen Textlayer und liefert Text, Seite und Bounding Box jedes Tokens. Aus diesen Tokens entstehen:

1. Zeilen anhand ihrer vertikalen Position,
2. Tabellenzellen anhand horizontaler Abstände,
3. Rechnungsfelder anhand beschrifteter Regeln,
4. Positionen anhand erkannter Spaltenanker.

Die Positionserkennung verwendet Überschriften wie Beschreibung, Menge, Einzelpreis und Gesamt als geometrische Anker. Sie kann dadurch auch Zeilen auflösen, in denen der PDF-Generator Beschreibung und Menge zu einem Texttoken zusammengezogen hat. Für Tabellen ohne Kopfzeile bleibt ein positionsbasierter Fallback aktiv.

Jedes Ergebnis behält sourceTokenIds, Quelltext und Confidence. Ein Klick auf das Eingabefeld markiert deshalb die zugehörige Stelle im PDF. Die gemeinsame Analyse in src/extraction/analyze.ts wird sowohl von der CLI als auch von der Desktop-App verwendet; Summen- und Positionsabweichungen werden an beiden Stellen identisch gemeldet.

Bei PDFs ohne ausreichenden Textlayer meldet die App den notwendigen OCR-Fallback. Die CLI kann dafür den lokalen Tesseract-Adapter verwenden.

## 3. Prüfung und kanonisches Modell

`src/review/draft.ts` übersetzt das Extraktionsergebnis in einen editierbaren `ReviewDraft`. Verkäufer, Käufer, Zahlung und Positionen gehören damit zu einem einzigen Zustand. Positionen können hinzugefügt oder entfernt werden. Der englische Typname ist nur eine interne Entwicklerbezeichnung; in der Oberfläche heißt der Schritt durchgängig „Prüfen“.

Geldwerte werden intern nie als JavaScript-number, sondern als kanonische Dezimalstrings gespeichert, zum Beispiel 1234.56. Der UI-Adapter akzeptiert und zeigt deutsche Schreibweise (1.234,56), wandelt sie beim Bearbeiten aber wieder in das kanonische Format um. Decimal.js übernimmt Multiplikation, Rundung, Steuergruppen und Summen.

Die Buttons **Für Behörden speichern** und **Als PDF-Rechnung speichern** hängen an getrennten Profilen von validateReviewDraft. Fehlende Pflichtangaben werden mit deutschen Feldnamen angezeigt. BR-DE-15 fordert die Käuferreferenz nur für XRechnung; ein ansonsten gültiger EN-16931-Draft darf ohne diesen Wert als ZUGFeRD exportiert werden. Erst ein für das jeweilige Format gültiger Draft wird in InvoiceInput umgewandelt und von EInvoiceEngine.calculate noch einmal berechnet und geprüft.

## 4. Ausgabe

**Entwurf speichern** sichert den Arbeitsbestand sofort und bestätigt Lernkorrekturen. Im Browser-Entwicklungsmodus bleibt ein JSON-Download mit Entwurf, Summen, Fehlern und Extraktionswarnungen verfügbar. **Für Behörden speichern** erzeugt eine XRechnung als UBL-2.1-XML. **Als PDF-Rechnung speichern** erzeugt EN-16931-CII, bettet sie als `factur-x.xml` mit `AFRelationship=Alternative` in die geöffnete PDF ein und ergänzt die Factur-X-XMP-Metadaten. Die sichtbaren PDF-Seiten bleiben erhalten.

In der nativen App schreibt Rust fertige Ausgaben als Bestandteil der Archivtransaktion in:

    Dokumente\E-Rechnung Ausgaben

Fertige XRechnungen und ZUGFeRD-PDFs durchlaufen genau ein gemeinsames Tauri-Kommando, das Ausgabe und Archivierung koordiniert; frühere parallele native XML-/PDF-Speicherkommandos wurden entfernt. Für ZUGFeRD akzeptiert Rust nur eine `.pdf` mit PDF-Signatur, eingebettetem `factur-x.xml`, `Alternative`-Beziehung und maximal 120 MiB. Dateiname, Erweiterung, Inhaltstyp und Größe werden auf der nativen Seite erneut geprüft. Bestehende gleichnamige Ausgaben werden nicht überschrieben, sondern durch eine nummerierte neue Datei ergänzt. Im reinen Browser-Entwicklungsmodus fällt derselbe UI-Flow auf einen normalen Download zurück; dort steht bewusst kein SQLite-Archiv zur Verfügung.

Das Einbetten allein konvertiert ein beliebiges Quelldokument nicht in PDF/A-3. Für formale ZUGFeRD-Konformität muss bereits die Quelle PDF/A-3 sein und das Ergebnis anschließend unabhängig mit veraPDF sowie den zum FeRD-Release gehörenden XML-Regeln validiert werden. Dieser Prototyp zeigt diese noch ausstehende Prüfung im Exportfeedback an.

## 5. Lokales Vorlagengedächtnis

Beim Öffnen einer Rechnung bleibt das unveränderte Extraktionsergebnis neben dem bearbeitbaren Entwurf erhalten. Erst wenn der Benutzer einen Entwurf oder eine fertige Rechnung speichert, vergleicht `src/learning/correction-memory.ts` den bestätigten Entwurf mit diesem Ausgangszustand. Für geänderte Werte sucht das Modul die zugehörige Textzeile im Quelldokument und speichert Feldart, relative Seitenposition sowie Text vor und nach dem Wert. Der konkrete Wert selbst wird nicht gespeichert. Dadurch kann beispielsweise aus `Auftragskennung: K-4711` eine Regel entstehen, die in der nächsten Rechnung `Auftragskennung: K-5000` ausliest, ohne `K-4711` im Gedächtnis abzulegen.

Regeln werden durch gehashte Absendermerkmale und eine Layoutsignatur begrenzt. Eine vorhandene, sicher erkannte Angabe wird nur dann ersetzt, wenn die Regel aus einer vorherigen ausdrücklichen Korrektur stammt. Vollständig fehlende Positionstabellen können analog über die relativen Spaltenpositionen von Bezeichnung, Menge, Einzelpreis und Betrag gelernt werden. Gelernte Ergebnisse behalten Quell-Token und werden in der Oberfläche als aus einer ähnlichen Rechnung übernommen gekennzeichnet.

Seit 0.2.1 kann die Oberfläche eine Feldquelle explizit auswählen. `PdfReview` übergibt die per Klick oder Rahmen ausgewählten Token-IDs; `App` übernimmt den geprüften Wert und reicht die Zuordnung beim Speichern an `learnCorrections` weiter. Eine bestätigte Quelle wird auch dann gelernt, wenn sich der Textwert nicht geändert hat. Neue Feldregeln besitzen `region: true` und begrenzen das Auslesen auf den konkreten Textblock. Die optionale `templateKey` wird aus mindestens vier festen Beschriftungen und deren Spalten gebildet; variable Namen und vertikale Umbrüche des Rechnungskörpers ändern sie nicht. Postleitzahl und Ort werden bei einem gemeinsamen PDF-Texttoken getrennt gelesen, ohne die bisherigen Werte als Beschriftung zu speichern. Alte Regeln ohne diese optionalen Eigenschaften bleiben lesbar und bevorzugen einzelne Textblöcke innerhalb ihrer früher gespeicherten Zeile.

Der Windows-Programmeinstieg verwendet das GUI-Subsystem auch für direkte Starts über den Drucker. Die Anwendung erzeugt dadurch kein zusätzliches Konsolenfenster.

Beim Öffnen eines Belegs im ursprünglichen Gültigkeitsbereich aktualisiert `upgradeLegacyFieldRules` alte Zeilenregeln auf konkrete Textblöcke und die neue Vorlagenkennung. Die bestehenden Felder, Zeitstempel und Regeln bleiben erhalten; nur die Quellzuordnung wird verfeinert. Die Aktualisierung wird über den bestehenden atomaren Speicherweg gesichert. Eine Regel wird nicht anhand einer beliebigen fremden Rechnung migriert.

Die React-Schicht lädt und speichert das Gedächtnis über zwei kleine Tauri-Kommandos. Rust begrenzt die Datei auf 1 MiB, prüft Version sowie die getrennten Obergrenzen für Feld- und Tabellenregeln und schreibt sie atomar in den plattformspezifischen lokalen Anwendungsordner. Im Browser-Entwicklungsmodus wird derselbe Inhalt in `localStorage` gehalten. Das Löschen in der Oberfläche ersetzt das Gedächtnis atomar durch eine leere Version.

## 6. Integriertes Rechnungsarchiv

Eine fertige Ausgabe wird erst als erfolgreich gemeldet, nachdem Rust sowohl die normale Ausgabedatei als auch einen Archiveintrag angelegt hat. Für jeden Eintrag werden genau zwei Belegdateien gespeichert:

    Dokumente\E-Rechnungsarchiv\Belege\<Jahr>\<Monat>\<UUID>\rechnung.pdf
    Dokumente\E-Rechnungsarchiv\Belege\<Jahr>\<Monat>\<UUID>\rechnungsdaten.xml

Bei XRechnung ist `rechnung.pdf` die sichtbare Quellrechnung und `rechnungsdaten.xml` die erzeugte UBL-Datei. Bei ZUGFeRD sind es die fertige Hybrid-PDF und deren separat abgelegte CII-Daten. Metadaten, relative Pfade und Integritätswerte liegen in `archiv.sqlite3`. SQLite verwendet Foreign Keys, WAL, `synchronous=FULL`, ein versioniertes Schema und eine `IMMEDIATE`-Transaktion, damit zwei parallele Ausgaben nicht denselben Kettenvorgänger erhalten. Neu angelegte Dateien werden synchronisiert; schlägt ein Schritt kontrolliert fehl, werden die nur für diesen Versuch erzeugten Dateien entfernt. Die Grenze zwischen SQLite und Dateisystem kann bei einem harten Prozess- oder Stromausfall naturgemäß nicht vollständig atomar sein. Die Archivprüfung meldet deshalb sowohl fehlende Dateien als auch Belegdateien ohne Datenbankeintrag. Für produktiven Betrieb gehört das Archiv auf ein gesichertes Dateisystem mit Backup und Wiederherstellungsprozedur.

PDF und XML erhalten jeweils eine SHA-256-Prüfsumme. Darüber liegt eine zweite Kette. Der Hash eines Eintrags wird aus einer längenpräfixierten, versionierten Folge aller unveränderlichen Metadaten, relativen Pfade, beider Dateihashes, der laufenden Nummer, dem Hash des Vorgängers und der optionalen Schlüsselkennung gebildet. `archive_state` hält Anzahl und Kopf der Kette fest. Die Prüfung liest jede Datei neu, prüft die SQLite-Integrität, lückenlose Nummerierung, alle Vorgängerbezüge, den gespeicherten Kettenkopf und jede vorhandene Signatur.

Optional erzeugt Rust einen Ed25519-Schlüssel im plattformspezifischen lokalen Anwendungsordner. Der private 32-Byte-Schlüssel verlässt den Rechner nicht; SQLite speichert nur den öffentlichen Schlüssel und dessen SHA-256-Kennung. Signiert wird der ASCII-kodierte Kettenhash. Dadurch bleiben bereits signierte Einträge auch dann mathematisch prüfbar, wenn der Schutz für neue Einträge später deaktiviert wird. Eine Schlüsselrotation und Betriebssystem-Keychain-Anbindung sind bewusst spätere Härtungsschritte.

Die Hash-Kette ist Manipulationserkennung, kein externer Vertrauensanker. Wer zugleich Schreibzugriff auf alle Dateien, die Datenbank und lokale Schlüssel besitzt, kann auch lokale Beweise angreifen. Unveränderbarer Objektspeicher, qualifizierte Zeitstempel, Berechtigungskonzept, Aufbewahrungsfristen und dokumentierte Verfahrensabläufe bleiben außerhalb des Anwendungs-Scope und dürfen durch die UI nicht behauptet werden.

Die Archivsuche arbeitet seitenweise mit höchstens 100 Treffern pro Ansicht. Dadurch bleiben Startzeit und Speicherbedarf auch bei einem über Jahre gewachsenen Archiv begrenzt; die frühere stille Obergrenze von 500 Einträgen existiert nicht mehr.

## Plattformgrenze

Domain, Extraktion, Review und XML-Engine sind TypeScript und laufen im Tauri-WebView unter Windows und macOS. Rust stellt das native Fenster, Single-Instance-Verhalten, Deep Links und sichere Dateizugriffe bereit. Nur die Druckannahme ist plattformspezifisch: Windows verwendet den Print-Support-Virtual-Printer; eine spätere macOS-Anbindung kann denselben Inbox-/Deep-Link-Vertrag über einen separaten Print-Workflow bedienen.

## Isolierte native Tests

Nur Debug-Builds akzeptieren den absoluten Pfad `ERECHNUNG_TEST_ROOT`. `paths.rs` lenkt damit Dokumente, lokalen und Roaming-Anwendungsordner auf getrennte Unterordner des Testprofils um; Release-Builds ignorieren diese Variable. `scripts/smoke-workspace.mjs` prüft Import, zehn parallel eingereihte Druckeingänge, Wiederaufnahme nach hartem Prozessabbruch, Fenstergrößen und das Sichern unmittelbar vor dem Schließen im echten WebView2-Fenster. Die Überwachungsschnittstelle wird dafür ausschließlich beim Teststart über `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` eingeschaltet. Der normale Produktstart benötigt weder diesen Port noch einen Webserver. Ablauf und Grenzen stehen in [wp7-acceptance.md](wp7-acceptance.md).
