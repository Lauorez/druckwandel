<p align="center"><img src="apps/native/assets/app-icon.svg" alt="Druckwandel" width="96"></p>
<h1 align="center">Druckwandel</h1>

Desktop-Anwendung in **Rust und Slint** für Windows und macOS: PDF-Rechnungen übernehmen, Daten am Original prüfen, **XRechnung (UBL)** oder **ZUGFeRD/Factur-X (PDF/A-3B)** erzeugen und per Systemfreigabe weitergeben.

## Funktionen

- **Import & Vorschau**: PDF-Import, eingebettete lokale PDF-Vorschau, lokale SQLite-Entwürfe und Windows-Druckeingang (`erechnung-review://`).
- **Erkennung**: Lokale Schemaerkennung in Rust mit Fundstellen, lokaler Tesseract-OCR für Scans und Lernprofilen; optionale Cloud-Erkennung über **W System** (eigener API-Schlüssel im System-Schlüsselbund, ausdrückliche Freigabe) sowie vorbereiteter Vertrag für eine spätere lokale KI. Alle Vorschläge bleiben prüfpflichtige Entwürfe ([Erkennungsarchitektur](docs/recognition.md)).
- **Rechnungsformular & Export**: Unterstützt Rechnungen, Gutschriften, Korrekturen, Teil-/Anzahlungs-/Schlussrechnungen, Steuerfälle, Rabatte/Zuschläge und Rechnungsbezüge. Ausgaben werden vor dem Speichern offline geprüft (KoSIT, Mustang, veraPDF); „Per E-Mail senden“ gibt nur unveränderte, geprüfte Dateien frei.
- **Installer**: Windows-Setup inkl. virtuellem .NET-Drucker; macOS-App als DMG.

## Branches

- **`development`** – laufende Entwicklung
- **`main`** – Release-Stand

## Entwicklung

Voraussetzungen: Rust (`rust-toolchain.toml`), Python 3.12+ für lokale Prüfer/OCR, unter Windows zusätzlich MSVC Build Tools (und NSIS für den Installer) sowie .NET 10 für den Drucker.

```sh
cargo run -p druckwandel-native --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/fetch-validators.py
cargo test --locked --test external_validators -- --ignored
```

CLI-Helfer (`druckwandel-recognize`, `druckwandel-xml`, `druckwandel-zugferd`) sowie lokale Installer-Skripte (`scripts/build-windows-installer.ps1`, `scripts/build-macos-installer.sh`) liegen im Repository. Weitere Details stehen unter [docs/](docs/README.md) und [resources/validators/](resources/validators/README.md).

## Verzeichnisse

| Verzeichnis | Inhalt |
| --- | --- |
| `apps/native/` | Rust-Fachkern, Slint-Oberfläche, Tests |
| `resources/validators/` | Manifest und lokale Java-Prüfer |
| `resources/ocr/` | Manifest für lokale Windows-OCR |
| `installer/windows/` | NSIS-Setup und Druckerinstallation |
| `printer/` | Virtueller Windows-Drucker (.NET NativeAOT) |
| `docs/` | Architektur, Produktplan und Audits |

Quellcode unter [MIT](LICENSE). Lizenzen eingebundener Komponenten siehe [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
