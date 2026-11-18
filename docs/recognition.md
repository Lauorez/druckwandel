# Schemaerkennung und vorbereitete lokale KI

## Gemeinsamer Vertrag

`apps/native/src/recognition/mod.rs` definiert `RecognitionProvider` und die Registry. Ein Anbieter meldet seine Methode (`schema` oder `local-ai`) und verarbeitet einen `RecognitionRequest`. Die App registriert standardmäßig ausschließlich `SchemaRecognizer`.

Der versionierte, mit Serde serialisierbare Request enthält:

- `schemaVersion = 1`;
- SHA-256 des Originaldokuments;
- den lokalen PDF-Pfad;
- Seiten mit Größe und Texttokens, jeweils mit stabiler ID, Seitenzahl und Bounding Box.

`RecognitionRequest::from_pdf` vergleicht vor der Erkennung den tatsächlichen Datei-Hash mit dem Hash des Imports. Der Request wird aus denselben gelesenen Bytes erzeugt. Die Seiten- und Tokenstruktur ist unabhängig vom gewählten Anbieter.

`RecognitionResult` enthält dieselbe Vertragsversion und Dokumentkennung, Methode und Anbieterversion, Feldvorschläge, Positionsvorschläge sowie Warnungen. Jeder Vorschlag enthält Konfidenz, referenzierte Token-IDs und den lesbaren Quelltext. Die Registry lehnt falsche Dokumentkennungen, falsche Methoden, unbekannte Vertragsversionen, ungültige Konfidenzen und Verweise auf unbekannte Tokens ab. Konfidenzen der Schemaerkennung sind Regelgewichte, keine statistisch kalibrierten Wahrscheinlichkeiten.

Ein Ergebnis ist ein **editierbarer Vorschlag**. Es überspringt weder die manuelle Prüfung noch Berechnung und unabhängige XML-/PDF-Prüfung. Erkennungsergebnis und Fundstellen werden lokal beim Entwurf gespeichert und bei erneutem Öffnen angezeigt. Erneute Erkennung füllt nur fehlende Werte und leere Positionslisten; vorhandene Korrekturen bleiben erhalten. Ausgabeformatwechsel und Änderungen sperren die E-Mail-Freigabe.

## Schemaanbieter

`extraction.rs` liest PDF-Zeichen mit Koordinaten über `pdf-extract`, gruppiert sie in Wörter und Zeilen und behält deren Fundstellen. `recognition/schema.rs` wendet Feldregeln, Datums- und Betragsnormalisierung, Adressblöcke und Tabellenregeln an. Es prüft erkannte Summen sowie Menge × Preis gegen den gedruckten Positionsbetrag und meldet Abweichungen. Nicht eingebettete Rechnungspreisstellen können aus gerundeten Druckwerten nicht rekonstruiert werden; solche Fälle benötigen manuelle Korrektur. Textarme Seiten werden bei aktivierter OCR lokal über Tesseract erkannt. Deutsche und englische Sprachdaten werden im Windows-Installer mitgeliefert; die Sprache ist einstellbar. Native Textseiten bleiben erhalten, gemischte PDFs werden seitenweise behandelt. OCR-Wörter besitzen PDF-Koordinaten und IDs mit `-ocr`; OCR-Vorschläge werden auf höchstens 80 % (Positionen 75 %) begrenzt und ausdrücklich zur Prüfung gekennzeichnet.

Beim Lesen normalisiert die App indirekte Parameter der Farbräume CalRGB/CalGray/Lab und überspringt Flate-komprimierte Inline-Bilder ausschließlich in einer internen Lesekopie. Die Bildgrenze wird anhand des vollständigen Zlib-Datenstroms ermittelt, nicht anhand vermuteter `EI`-Zeichen in Binärdaten. Originaldatei, Seiteninhalt für den Export und eingebettete Anhänge bleiben unverändert. Synthetische Regressionstests benötigen keine echten Rechnungsdaten.

Bereits vorhandene `factur-x.xml` wird erkannt und in der Oberfläche angezeigt, aber derzeit nicht als strukturierter Rechnungsentwurf importiert. Die Schemaerkennung verwendet auch bei solchen Belegen den sichtbaren PDF-Text. Fehlende Pflichtangaben und nicht erkannte Positionstabellen werden ausdrücklich gemeldet. Ein gültiger ZUGFeRD-Beleg kann daher aktuell nur unvollständige Textvorschläge liefern; der direkte CII-Import ist noch offen.

Beim ersten Öffnen eines neuen Dokuments läuft die gewählte Erkennung im Hintergrund. Die Oberfläche bleibt aktiv; Änderungen und Export sind während des Laufs gesperrt, damit Ergebnisse keine zwischenzeitlichen Korrekturen überschreiben. Fundstellen einschließlich OCR-Text werden lokal beim Entwurf gespeichert und beim Wiederöffnen geladen. Ein vorhandener Entwurf wird wiederhergestellt. Der Button „Fehlende Felder erkennen“ verwendet die aktuell gewählte Methode.

## Lokale OCR und Lernprofile

`ocr.rs` erkennt nur textarme Seiten, rendert mit Hayro und liest Tesseract-TSV mit Wortkoordinaten. Originaldateien werden nicht verändert. PNG/TSV liegen ausschließlich im temporären Arbeitsordner und werden nach dem Lauf entfernt. Grenzen: 40 Seiten, 4 Millionen Pixel je gerenderter Seite, 30 Sekunden je OCR-Prozess und 120 Sekunden je Dokument. Fehler lassen die manuelle Erfassung und Vorschau verfügbar. Paket und Quellen stehen unter [resources/ocr](../resources/ocr/README.md).

Unter Einstellungen → Erkennung werden OCR, Sprache und Lernprofile verwaltet. Nach dem Korrigieren einer Rechnung übernimmt „Aus Korrekturen lernen“ ausschließlich Werte, die eindeutig im aktuellen Beleg gefunden werden. Das aktive Profil lernt relative Fundstellen mit Textankern sowie Tabellenspalten. Rechnungsnummern und Datumswerte werden beim nächsten Beleg aus dessen Tokens gelesen; die zuvor bestätigten Werte werden nicht kopiert. Lieferantenmerkmale und Layoutanker begrenzen die Anwendung auf passende Belege. Mehrdeutige oder nicht im Quelltext enthaltene Korrekturen werden nicht als Feldregel gespeichert. Gelernte Tabellen verlangen nachvollziehbare Mengen-/Preis-/Betragsangaben, unterstützen einen gemeinsamen bestätigten Steuerfall und müssen manuell geprüft werden.

Bis zu 20 Profile lassen sich erstellen, auswählen, umbenennen und löschen. Einzelne Regeln können deaktiviert oder entfernt und die letzte Speicheränderung rückgängig gemacht werden. Regeln werden lokal und atomar in `native/learning-profiles.json` gespeichert; OCR-Einstellungen stehen in `settings.json`. Die neue Rust-Profildatei ist von den alten Tauri-Profilen getrennt; eine automatische Migration alter Profil-JSON-Dateien erfolgt nicht. Profile wirken auf neue Vorschläge. Erneute Erkennung füllt weiterhin nur fehlende Felder und bewahrt manuelle Korrekturen.

## Spätere KI aus einem separaten Repository

Die Auswahl „Lokale KI (noch nicht installiert)“ ist sichtbar. Solange kein Anbieter registriert ist, erscheint eine eindeutige Meldung. Es gibt keinen stillen Wechsel zur Schemaerkennung und keine Änderung am Entwurf.

Das spätere KI-Projekt stellt einen Adapter bereit, der `RecognitionProvider` für `RecognitionMethod::LocalAi` implementiert. Die App bindet diesen am zentralen Kompositionspunkt `application_registry()` durch `RecognitionRegistry::register` ein. Diese Registry wird einmal initialisiert und bereits heute gemeinsam von Import und Erkennungsbutton verwendet. Ob der Adapter später als Rust-Crate eingebunden oder ein separater lokaler Prozess angebunden wird, entscheidet die Integration des KI-Projekts. Der serialisierbare Vertrag kann für beide Varianten verwendet werden.

Kleines LLM, LoRA-Adapter, Inferenzlaufzeit, Modellverwaltung und Modelltests gehören vollständig in das separate Projekt. Dieses Repository lädt kein Modell, startet keinen KI-Dienst, enthält keinen Transportadapter und ruft keine externe API auf. Ein späterer Adapter muss vollständig lokal arbeiten; Netzwerkanbieter sind kein vorgesehener Betriebsmodus.

Die fachliche Ausgabe bleibt unabhängig vom Anbieter: Beide liefern denselben Entwurf an Berechnung, manuelle Korrektur und KoSIT beziehungsweise Mustang/veraPDF.

## Prüfbarkeit

`tests/recognition.rs` prüft die Erkennung am echten Muster-PDF, Fundstellen, das Bewahren manueller Korrekturen und die Ablehnung eines Anbieterergebnisses für ein anderes Dokument. Ein Testadapter verwendet keine KI. Unit-Tests sichern den ausdrücklich nicht verfügbaren KI-Zustand ab.

```sh
cargo run -p druckwandel-native --bin druckwandel-recognize -- quelle.pdf
cargo test -p druckwandel-native --locked --test recognition --test learning --test ocr
python scripts/fetch-ocr.py
cargo test -p druckwandel-native --locked --test ocr -- --ignored
```
