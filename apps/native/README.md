# Druckwandel in Rust und Slint

Dies ist die einzige Desktop-App auf `main`. Einstieg ist `src/main.rs`, die Oberfläche liegt unter `ui/`, die Fachlogik in der Rust-Bibliothek.

```sh
cargo run -p druckwandel-native --locked
cargo test -p druckwandel-native --locked
python3 scripts/fetch-validators.py
cargo test -p druckwandel-native --locked --test external_validators -- --ignored
```

Der Anwendungskern übernimmt PDF-Text mit Positionen, Schemaerkennung, Entwürfe, XRechnung, ZUGFeRD und die Offline-Prüfer. Beide Ausgabeformate werden erst nach erfolgreicher Prüfung gespeichert. Der E-Mail-Button gibt ausschließlich die unveränderte, gespeicherte Ausgabe an die Systemfreigabe weiter.

Die [Erkennungsarchitektur](../../docs/recognition.md) beschreibt die vorbereitete Schnittstelle zum späteren separaten lokalen KI-Projekt. Ein KI-Backend oder Modell ist nicht enthalten.

Der Windows-Drucker verwendet weiterhin `erechnung-review://print-job/<UUID>`. `installer/windows/druckwandel.nsi` registriert die Slint-EXE. Eine Dateisperre erzwingt eine Instanz je lokalem App-Datenordner; weitere Aufrufe landen in einer lokalen SQLite-Übergabewarteschlange.

Die PDF-Vorschau verwendet Hayro 0.8 direkt in Rust. Ein Hintergrund-Worker hält nur ein Dokument und rendert jeweils eine Seite; die Ausgabe ist auf 4 Millionen Pixel begrenzt. Seitenwechsel und Zoom verdrängen wartende ältere Aufträge. Rasterbilder sind ausschließlich im RAM; Original-PDFs werden nicht verändert. Die Texterkennung und die Vorschau können unabhängig voneinander fehlschlagen.

Über das Zahnrad sind Darstellung (System/Hell/Dunkel) und Über-Informationen erreichbar. Rust 1.92 ist für den Renderer erforderlich.

Noch nicht portiert sind OCR und Lernprofile. Die Oberfläche deckt einfache Rechnungen ab; zusätzliche fachliche Exportfälle werden über die Referenzentwürfe im Rust-Fachkern geprüft. [Projektübersicht](../../README.md).
