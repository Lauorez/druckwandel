# Änderungen

## 1.0.1

- Korrigiert den maximierten Fensterstart unter Windows: Die Maximierung wird erst nach dem Anlegen und initialen Sizing des nativen Fensters ausgelöst, sodass die gesamte Arbeitsfläche genutzt wird und das Fenster beim Ziehen sauber auf `1320×760` statt `900×600` zurückkehrt.

## 1.0.0

- Optionale Cloud-Erkennung über **W System** (`POST /einvoice/extract`, `GET /billing/usage`, Pay-as-you-go-Link, sichere API-Schlüssel-Speicherung im System-Schlüsselbund, ausdrückliche Datenschutz-Freigabe sowie automatischer lokaler Tesseract-OCR-Fallback für gescannte PDFs ohne Textebene).
- Signierte Auto-Updates (Ed25519 + SHA-256) mit Update-Prüfung unter *Einstellungen → Allgemein*, Windows-Silent-Update-Übergabe und automatisiertem GitHub-Release-Workflow für Windows und macOS.
- Dropdown-Auswahlfelder mit „Sonstiges“-Fallback für ISO-Ländercodes, Währung (ISO 4217), Zahlungsart (UNTDID 4461) und Mengeneinheiten (UN/ECE Rec. 20) sowie modaler Prüfhinweis nach abgeschlossener Rechnungserkennung.
- Eingebettetes Windows-Anwendungs- und Installer-Icon, zuverlässig maximierter Programmstart unter Windows sowie gepinntes Eclipse Temurin 21 JRE für die Offline-Prüfer.

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
