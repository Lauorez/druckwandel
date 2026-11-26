# Schemaerkennung, vorbereitete lokale KI und optionale W-System-Cloud-Erkennung

## Gemeinsamer Vertrag

`apps/native/src/recognition/mod.rs` definiert `RecognitionProvider` und die Registry. Ein Anbieter meldet seine Methode (`schema`, `local-ai` oder `wsystem`) und verarbeitet einen `RecognitionRequest`. `RecognitionRegistry::default()` registriert standardmäßig ausschließlich `SchemaRecognizer`, sodass ohne ausdrückliche Opt-in-Auswahl keine Netzwerkanfragen erfolgen.

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

Bis zu 20 Profile lassen sich erstellen, auswählen, umbenennen und löschen. Einzelne Regeln können deaktiviert oder entfernt und die letzte Speicheränderung rückgängig gemacht werden. Regeln werden lokal und atomar in `native/learning-profiles.json` gespeichert; OCR-Einstellungen stehen in `settings.json`. Profile wirken auf neue Vorschläge. Erneute Erkennung füllt weiterhin nur fehlende Felder und bewahrt manuelle Korrekturen.

## Spätere KI aus einem separaten Repository

Die Auswahl „Lokale KI (noch nicht installiert)“ ist sichtbar. Solange kein Anbieter registriert ist, erscheint eine eindeutige Meldung. Es gibt keinen stillen Wechsel zur Schemaerkennung und keine Änderung am Entwurf.

Das spätere KI-Projekt stellt einen Adapter bereit, der `RecognitionProvider` für `RecognitionMethod::LocalAi` implementiert. Die App bindet diesen am zentralen Kompositionspunkt `application_registry()` durch `RecognitionRegistry::register` ein. Diese Registry wird einmal initialisiert und bereits heute gemeinsam von Import und Erkennungsbutton verwendet. Ob der Adapter später als Rust-Crate eingebunden oder ein separater lokaler Prozess angebunden wird, entscheidet die Integration des KI-Projekts. Der serialisierbare Vertrag kann für beide Varianten verwendet werden.

Kleines LLM, LoRA-Adapter, Inferenzlaufzeit, Modellverwaltung und Modelltests gehören vollständig in das separate Projekt. Dieses Repository lädt kein Modell, startet keinen lokalen KI-Dienst und ruft im lokalen Modus keine externe API auf. Ein späterer lokaler KI-Adapter muss vollständig lokal arbeiten.

## Optionale Cloud-Erkennung über W System

Zusätzlich zur standardmäßigen lokalen Schemaerkennung kann unter **Einstellungen → Erkennung** optional **W System (Cloud, API-Schlüssel)** (`RecognitionMethod::WSystem`, `apps/native/src/recognition/wsystem.rs`) gewählt werden.

### Datenschutz, Schlüsselverwaltung und Einwilligung

- **Opt-in und Sichtbarkeit**: Cloud-Erkennung ist standardmäßig inaktiv. Sowohl in den Einstellungen als auch im Reiter **Erkennung** der Rechnungsansicht weist die Oberfläche sichtbar darauf hin, dass die gewählte PDF-Rechnung verschlüsselt (HTTPS) an W System (`https://api.wsystem.dev`) übermittelt wird und W System den PDF-Text zur KI-Extraktion an OpenAI als Unterauftragsverarbeiter weitergibt (AVV/DPA: `https://wsystem.dev/legal/dpa`).
- **Einwilligung & manuelle Auslösung**: Ohne gespeicherten API-Schlüssel und ausdrücklich aktivierte Einwilligungs-Checkbox sendet die App keine Datei. Auch nach der Aktivierung bleibt das automatische Senden beim Öffnen neuer PDFs standardmäßig ausgeschaltet (`auto_recognize = false`), damit weder unbeabsichtigte Übermittlungen noch unerwartete Kosten entstehen.
- **Geheimnisschutz**: Der eigene W-System-API-Schlüssel (über `https://api.wsystem.dev/account/free-key?ref=druckwandel` erhältlich) wird ausschließlich im Betriebssystem-Schlüsselbund gespeichert (`apps/native/src/secrets.rs`: Windows Credential Manager bzw. macOS Keychain), niemals in `settings.json`, `workspace.sqlite3`, Protokollen oder Fehlermeldungen. Das Eingabefeld ist maskiert und wird nach dem Speichern sofort geleert. Weiterleitungen (`3xx`) werden deaktiviert, damit `X-API-Key` nicht weitergegeben werden kann.
- **Preishinweis**: Für Druckwandel-Nutzer ist die Pay-as-you-go-Abrechnung unter `https://api.wsystem.dev/billing/druckwandel` aktiv: 0,15 € je erfolgreicher Rechnung ohne Abo, Mindestumsatz oder Setup-Gebühr (monatlich nach Verbrauch, fehlgeschlagene Extraktionen kostenlos). Der kostenlose Testschlüssel über `https://api.wsystem.dev/account/free-key?ref=druckwandel` enthält 3 Testextraktionen/Monat (80 Einheiten je erfolgreicher Extraktion).

### API-Vertrag, Mapping und Live-Verifizierung

Verifizierte Quellen: `https://api.wsystem.dev/openapi.json`, `https://wsystem.dev/pdf-to-xrechnung`, `https://wsystem.dev/docs`, `https://api.wsystem.dev/einvoice/sample`, `https://wsystem.dev/legal/dpa`, die offiziellen W-System-Clients (`wsystem-python`, `shopware-e-invoice`) sowie die Live-Verifizierung am 26.11.2026 gegen `GET /billing/usage`, `POST /einvoice/extract?format=xrechnung` und `GET /billing/druckwandel`.

- **Extraktion**: `POST https://api.wsystem.dev/einvoice/extract?format=xrechnung` mit `X-API-Key`, `Content-Type: application/pdf` (verifizierte Original-PDF-Bytes, höchstens 10 MB), `Accept: application/json`, `X-Request-Id` und `Idempotency-Key`. Bestehende ZUGFeRD-/Factur-X-PDFs (`422 already_einvoice`) werden serverseitig abgelehnt und klar gemeldet. Enthält eine PDF keine vollständige Textebene (Scan) oder meldet W System `422 no_text_layer`, extrahiert Druckwandel den Text lokal per Tesseract-OCR, erzeugt daraus im Speicher eine temporäre Text-PDF und übermittelt diese an W System (das Original-PDF bleibt unverändert).
- **Kontingentabfrage**: `GET https://api.wsystem.dev/billing/usage` mit `X-API-Key` und `X-Request-Id` (kostenlos, verbraucht keine Einheiten). Die Live-Antwort liefert `plan`, `quota`, `used`, `remaining`, `period` sowie `unit_weights["/einvoice/extract"] = 80`. Zusätzlich werden Antwort-Header `X-Units-Used` und `X-Units-Remaining` ausgewertet.
- **Request-ID für Support**: Jede Anfrage sendet eine `X-Request-Id` und übernimmt die Server-`X-Request-ID` bzw. `request_id` aus dem Antwort-Body. Sie wird im Prüfprotokoll (`RecognitionResult.request_id` / `review_text()`) und in jeder Fehlermeldung sichtbar angezeigt.
- **Zuordnung & Prüfablauf**: Erkannte Felder, Positionen, Steuerfälle (inkl. Top-Level-`vat_breakdown`), Zu-/Abschläge, Rechnungsbezüge, Konfidenzen, `arithmetic_check`-Summen, `review`-Hinweise (`low_confidence`, `model_uncertainties`, `input_error`, `blocked`) und Warnungen werden auf `RecognitionResult` und `InvoiceInput` abgebildet. Gedruckte Dokumentensummen aus `arithmetic_check` (`sum of line nets (BT-106/109)`, `VAT total (BT-110)`, `total with VAT (BT-112)`, `amount due (BT-115)`) werden vor den berechneten XML-/Top-Level-`totals`-Werten übernommen und greifen in Druckwandels Export-Summenprüfung. Alle Werte bleiben editierbare Vorschläge; erneute Erkennung bewahrt manuelle Korrekturen; lokale Berechnung sowie KoSIT-/Mustang-/veraPDF-Prüfung bleiben verpflichtend.
- **Ergebnisse der Live-Verifizierung (26.11.2026)**:
  1. `openapi.json` deklariert für beide 200-Antworten zwar noch `schema: {}`, im Live-Betrieb liefert `POST /einvoice/extract?format=xrechnung` jedoch zuverlässig Top-Level-`invoice`, 0-basierte `fields[]`-Pfade (`lines.0.*`), `vat_breakdown[]`, `totals` (`line_total`, `vat_total`, `total_with_vat`, `payable`), `review` (`low_confidence`, `min_confidence`, `model_uncertainties`, `input_error`, `blocked`), `arithmetic_check` (sowohl Positionszeilen `lines.N: ...` als auch Dokumentensummen `BT-106/109`, `BT-110`, `BT-112`, `BT-115` plus `note`), `validation` und UBL-2.1-`xml`, zusammen mit den Headern `X-Request-ID`, `X-Units-Used` und `X-Units-Remaining`.
  2. Der Mapper filtert Positionszeilen (`lines.N: ...`) in `arithmetic_check` sauber von den Dokumentensummen und wertet gedruckte Summen vor dem UBL-XML-Fallback aus, sodass auch 1-Cent-Rundungsabweichungen am Originalbeleg zuverlässig erkannt werden.
  3. Defensive Fallbacks (`draft`/`extracted`, 1-basierte `fields[]`, UBL-XML-Fallback) bleiben erhalten.

Die fachliche Ausgabe bleibt unabhängig vom Anbieter: Alle Erkennungswege liefern denselben Entwurf an Berechnung, manuelle Korrektur und KoSIT beziehungsweise Mustang/veraPDF.

## Prüfbarkeit

`tests/recognition.rs` prüft die Erkennung am echten Muster-PDF, Fundstellen, das Bewahren manueller Korrekturen und die Ablehnung eines Anbieterergebnisses für ein anderes Dokument. `tests/wsystem.rs` prüft die W-System-Integration vollständig offline gegen einen lokalen Mock-HTTP-Server (Extraktion, `needs_review_amounts`, Gutschrift/Kleinunternehmer, UBL-XML-Fallback, Fehlercodes mit Request-ID, Kontingentabfrage, Geheimnisschutz und Sicherheitsgrenzen). Ein Testadapter verwendet keine KI. Unit-Tests sichern den ausdrücklich nicht verfügbaren KI-Zustand ab.

```sh
cargo run -p druckwandel-native --bin druckwandel-recognize -- quelle.pdf
cargo test -p druckwandel-native --locked --test recognition --test wsystem --test learning --test ocr
python scripts/fetch-ocr.py
cargo test -p druckwandel-native --locked --test ocr -- --ignored
```
