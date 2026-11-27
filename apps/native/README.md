# Druckwandel in Rust und Slint

Dies ist die einzige Desktop-App auf `main`. Einstieg ist `src/main.rs`, die Oberfläche liegt unter `ui/`, die Fachlogik in der Rust-Bibliothek.

```sh
cargo run -p druckwandel-native --locked
cargo test -p druckwandel-native --locked
python3 scripts/fetch-validators.py
cargo test -p druckwandel-native --locked --test external_validators -- --ignored
```

Der Anwendungskern übernimmt PDF-Text mit Positionen, Schemaerkennung, Entwürfe, XRechnung, ZUGFeRD und die Offline-Prüfer. Beide Ausgabeformate werden erst nach erfolgreicher Prüfung gespeichert. Der E-Mail-Button gibt ausschließlich die unveränderte, gespeicherte Ausgabe an die Systemfreigabe weiter.

Die [Erkennungsarchitektur](../../docs/recognition.md) beschreibt die lokale Schemaerkennung, die vorbereitete Schnittstelle zum späteren separaten lokalen KI-Projekt sowie die optionale Cloud-Erkennung über W System (eigener API-Schlüssel im System-Schlüsselbund, ausdrückliche Freigabe). Ein lokales KI-Modell ist nicht enthalten.

Der Windows-Drucker verwendet weiterhin `erechnung-review://print-job/<UUID>`. `installer/windows/druckwandel.nsi` registriert die Slint-EXE. Eine Dateisperre erzwingt eine Instanz je lokalem App-Datenordner; weitere Aufrufe landen in einer lokalen SQLite-Übergabewarteschlange.

Die PDF-Vorschau verwendet Hayro 0.9 direkt in Rust. Ein Hintergrund-Worker hält nur ein Dokument und rendert jeweils eine Seite; die Ausgabe ist auf 4 Millionen Pixel begrenzt. Seitenwechsel und Zoom verdrängen wartende ältere Aufträge. Vorschaubilder sind ausschließlich im RAM; Original-PDFs werden nicht verändert. Die Texterkennung und die Vorschau können unabhängig voneinander fehlschlagen.

Die App startet maximiert und passt Schrift, Feldbreiten und Spalten an Fenstergröße und Windows-DPI an. ZUGFeRD ist die vorausgewählte Ausgabe; Speichern und Weitergabe stehen unter dem Formular. Leere Pflichtfelder werden rot markiert, zusätzliche XRechnung-Pflichtfelder abhängig vom gewählten Format.

Über das Zahnrad sind die Reiter Unternehmen, Allgemein, Erkennung und Über erreichbar. Unternehmensdaten werden lokal in `settings.json` gespeichert und ergänzen fehlende Angaben neuer Entwürfe. Die ausdrückliche Übernahme in die aktuelle Rechnung ersetzt deren Rechnungssteller und IBAN durch die gespeicherte Vorlage. Darstellung (System/Hell/Dunkel) und Erkennungsmodus werden ebenfalls lokal gespeichert; die bisherige `appearance.json` wird beim ersten Laden übernommen. Ein Seitenwechsel verändert die Fenstergröße nicht. Rust 1.92 ist für den Renderer erforderlich.

Das Rechnungsformular ist in Rechnung, Positionen und Details gegliedert. Es bietet Rechnung/Gutschrift/Korrektur/Teilrechnung sowie Anzahlungs- und Schlussrechnung, Leistungsdatum, Währung, Mengeneinheiten, die fünf unterstützten Steuerkategorien einschließlich Befreiungsgrund, Positions- und Rechnungsrabatte/-zuschläge und mehrere Rechnungsbezüge. Zahlungsbedingungen, Zahlungsart, Bankangaben, Hinweise und bereits gezahlte Beträge sind bearbeitbar. Positionsrabatte übernehmen den Steuerfall ihrer Position. Prozent- und Basisangaben müssen zum ausdrücklich eingegebenen Nettobetrag passen; sie lösen keine zusätzliche Betragsberechnung aus.

Ein Formular-Rundlauf über alle elf Referenzfälle prüft unveränderte UBL- und CII-Ausgaben einschließlich Positionswechsel. Hinzufügen/Entfernen und die dauerhafte Speicherung von Rabatten und Rechnungsbezügen werden ebenfalls geprüft. OCR für textarme Seiten läuft lokal über Tesseract. Sprache und Lernprofile werden im Reiter Erkennung verwaltet; „Aus Korrekturen lernen“ speichert Fundstellen und Tabellenspalten für passende Folgebelege. OCR und Lernen laufen im Hintergrund. Vorschläge und ihre Textfundstellen werden lokal beim Entwurf gespeichert. Das Windows-OCR-Paket wird mit `python scripts/fetch-ocr.py` vor dem Installerbau bereitgestellt. Das separate KI-Modell ist weiterhin nicht enthalten. [Projektübersicht](../../README.md).
