# Änderungen

## 0.4.0

- Lokale Tesseract-OCR für textarme und gemischte PDFs, deutscher/englischer Offline-Laufzeit im Windows-Installer, Hintergrund-Erkennung und gespeicherten Fundstellen.
- Verwaltbare Lernprofile für quellengestützte Feld- und Tabellenzuordnungen mit Deaktivierung, Entfernen und Rückgängig.

- Rechnungsformular um Rechnungsarten, Leistungsdatum, Währung, Mengeneinheiten, Steuerfälle, Rabatte/Zuschläge und mehrere Rechnungsbezüge erweitert; Formular in Rechnung, Positionen und Details gegliedert.

- ZUGFeRD als Standardausgabe, Ausgabeaktionen unter dem Formular und sichtbare Pflichtfeldmarkierungen.
- Einstellungen mit Reitern für Unternehmensvorlage, Darstellung, Erkennungsmodus und Über-Informationen; maximierter Start und anpassbare Darstellung ohne Größenänderung beim Seitenwechsel.

- Rust/Slint als einzige App auf `main`; bisheriger Stand im gesperrten Branch `legacy/tauri-0.3.5`.
- PDF-Import, Entwürfe und Schemaerkennung mit Fundstellen in Rust.
- Gemeinsamer Erkennungsvertrag und reservierte Auswahl für einen späteren lokalen KI-Adapter aus einem separaten Projekt.
- XRechnung und ZUGFeRD mit KoSIT, Mustang und veraPDF; bytegleiche eingebettete XML-Datei und unveränderte PDF-Seiten.
- Eigenständiger Windows-Installer mit Drucker und Protokollhandler, macOS-App-Bundle und DMG.
- E-Mail-Weitergabe der unveränderten gespeicherten Ausgabe.
- Archiv, Buchungsexport, Backup-Oberfläche und frühere Web-Buildkette entfernt.

Frühere Änderungen stehen im [Legacy-Changelog](https://github.com/Lauorez/druckwandel/blob/legacy/tauri-0.3.5/CHANGELOG.md).
