# Änderungen

## 0.4.1

- Der Windows-Installer bietet eine optionale Desktopverknüpfung.
- Die Seitenleiste lässt sich über die Kopfzeile ein- und ausblenden.
- Schnellere Navigation zwischen geöffneten Entwürfen und Rechnungspositionen sowie klarere Hinweise beim Export.
- Das Rechnungsformular akzeptiert Dezimalkommas. Ausgabedateinamen und Speicherorte werden vor dem Export geprüft.
- Die Audit-Korrekturen verbessern die Prüfung von PDF-Originalen und Ausgabeformaten sowie die Sicherheit beim Import und Speichern.

## 0.4.0

- Lokale Tesseract-OCR für textarme und gemischte PDFs, deutscher/englischer Offline-Laufzeit im Windows-Installer, Hintergrund-Erkennung und gespeicherten Fundstellen.
- Verwaltbare Lernprofile für quellengestützte Feld- und Tabellenzuordnungen mit Deaktivierung, Entfernen und Rückgängig.

- Rechnungsformular um Rechnungsarten, Leistungsdatum, Währung, Mengeneinheiten, Steuerfälle, Rabatte/Zuschläge und mehrere Rechnungsbezüge erweitert; Formular in Rechnung, Positionen und Details gegliedert.

- ZUGFeRD als Standardausgabe, Ausgabeaktionen unter dem Formular und sichtbare Pflichtfeldmarkierungen.
- Einstellungen mit Reitern für Unternehmensvorlage, Darstellung, Erkennungsmodus und Über-Informationen; maximierter Start und anpassbare Darstellung ohne Größenänderung beim Seitenwechsel.

- Rust/Slint als einzige Desktop-Anwendung.
- PDF-Import, Entwürfe und Schemaerkennung mit Fundstellen in Rust.
- Gemeinsamer Erkennungsvertrag und reservierte Auswahl für einen späteren lokalen KI-Adapter aus einem separaten Projekt.
- XRechnung und ZUGFeRD mit KoSIT, Mustang und veraPDF; bytegleiche eingebettete XML-Datei und unveränderte PDF-Seiten.
- Eigenständiger Windows-Installer mit Drucker und Protokollhandler, macOS-App-Bundle und DMG.
- E-Mail-Weitergabe der unveränderten gespeicherten Ausgabe.
- Archiv, Buchungsexport, Backup-Oberfläche und frühere Web-Buildkette entfernt.
