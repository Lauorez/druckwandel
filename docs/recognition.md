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

`extraction.rs` liest PDF-Zeichen mit Koordinaten über `pdf-extract`, gruppiert sie in Wörter und Zeilen und behält deren Fundstellen. `recognition/schema.rs` wendet Feldregeln, Datums- und Betragsnormalisierung, Adressblöcke und Tabellenregeln an. Es prüft erkannte Summen sowie Menge × Preis gegen den gedruckten Positionsbetrag und meldet Abweichungen. Nicht eingebettete Rechnungspreisstellen können aus gerundeten Druckwerten nicht rekonstruiert werden; solche Fälle benötigen manuelle Korrektur. PDFs ohne Text werden als manuell zu erfassen gekennzeichnet. OCR und die alten Lernprofile sind noch nicht portiert.

Beim ersten Öffnen eines neuen Dokuments läuft die Schemaerkennung. Ein vorhandener Entwurf wird wiederhergestellt. Der Button „Fehlende Felder erkennen“ verwendet die aktuell gewählte Methode.

## Spätere KI aus einem separaten Repository

Die Auswahl „Lokale KI (noch nicht installiert)“ ist sichtbar. Solange kein Anbieter registriert ist, erscheint eine eindeutige Meldung. Es gibt keinen stillen Wechsel zur Schemaerkennung und keine Änderung am Entwurf.

Das spätere KI-Projekt stellt einen Adapter bereit, der `RecognitionProvider` für `RecognitionMethod::LocalAi` implementiert. Die App bindet diesen am zentralen Kompositionspunkt `application_registry()` durch `RecognitionRegistry::register` ein. Diese Registry wird einmal initialisiert und bereits heute gemeinsam von Import und Erkennungsbutton verwendet. Ob der Adapter später als Rust-Crate eingebunden oder ein separater lokaler Prozess angebunden wird, entscheidet die Integration des KI-Projekts. Der serialisierbare Vertrag kann für beide Varianten verwendet werden.

Kleines LLM, LoRA-Adapter, Inferenzlaufzeit, Modellverwaltung und Modelltests gehören vollständig in das separate Projekt. Dieses Repository lädt kein Modell, startet keinen KI-Dienst, enthält keinen Transportadapter und ruft keine externe API auf. Ein späterer Adapter muss vollständig lokal arbeiten; Netzwerkanbieter sind kein vorgesehener Betriebsmodus.

Die fachliche Ausgabe bleibt unabhängig vom Anbieter: Beide liefern denselben Entwurf an Berechnung, manuelle Korrektur und KoSIT beziehungsweise Mustang/veraPDF.

## Prüfbarkeit

`tests/recognition.rs` prüft die Erkennung am echten Muster-PDF, Fundstellen, das Bewahren manueller Korrekturen und die Ablehnung eines Anbieterergebnisses für ein anderes Dokument. Ein Testadapter verwendet keine KI. Unit-Tests sichern den ausdrücklich nicht verfügbaren KI-Zustand ab.

```sh
cargo run -p druckwandel-native --bin druckwandel-recognize -- quelle.pdf
cargo test -p druckwandel-native --locked --test recognition
```
