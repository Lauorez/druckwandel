<p align="center"><img src="apps/native/assets/app-icon.svg" alt="Druckwandel" width="96"></p>
<h1 align="center">Druckwandel</h1>

Desktop-Anwendung in **Rust und Slint** für Windows und macOS. PDF-Rechnungen übernehmen, erkannte Daten prüfen, XRechnung oder ZUGFeRD erzeugen, speichern und per Systemfreigabe an ein Mailprogramm übergeben.

`main` enthält ausschließlich die neue Anwendung. Der bisherige Stand ist im gesperrten Branch [legacy/tauri-0.3.5](https://github.com/Lauorez/druckwandel/tree/legacy/tauri-0.3.5) eingefroren. Die veröffentlichten 0.3.x-Downloads gehören zu diesem alten Stand. Die neue Version 0.4.0 ist noch in Entwicklung.

## Umfang

- PDF-Import und Windows-Druckeingang mit lokalen SQLite-Entwürfen.
- Schemaerkennung in Rust: PDF-Text mit Fundstellen, Feldregeln, Adress- und Tabellenerkennung; Vorschläge mit Konfidenz und Warnungen.
- Auswahl einer später ergänzbaren **lokalen KI-Erkennung**. Das separate KI-Projekt mit kleinem LLM und LoRA-Adapter ist hier weder implementiert noch enthalten; es gibt keinen externen API-Aufruf. [Erkennungsarchitektur](docs/recognition.md).
- Bearbeitbare Rechnungsdaten und mehrere Positionen.
- XRechnung (UBL) und ZUGFeRD/Factur-X EN 16931 (CII in PDF/A-3B), mit unabhängig geprüften Ausgabebytes.
- „Per E-Mail senden“ erst nach erfolgreichem Speichern; Änderungen sperren die Freigabe erneut. Der Nutzer wählt das Mailziel und versendet selbst.
- Windows-Installer mit bestehendem .NET-Drucker und `erechnung-review://`-Protokollhandler; macOS-App als DMG.

Archiv, Buchungsexport, DATEV-Belegpakete und Backup-Oberfläche sind entfernt. Ein späterer DATEV-Belegupload ist im [Produktplan](docs/planning/slint-rust-umstieg.md) beschrieben.

Die Oberfläche wird weiter ausgebaut: eine eingebettete visuelle PDF-Vorschau, OCR, Lernprofile und vollständige Bedienelemente für alle Sonderfälle sind noch nicht portiert. Die Fachkern-Regressionen enthalten die elf bisherigen Exportfälle. Automatische Erkennung liefert Vorschläge; sachliche Richtigkeit und Übereinstimmung mit dem Beleg müssen geprüft werden.

## Entwicklung

Rust-Version siehe `rust-toolchain.toml`. Python 3.12+ wird zum Laden der unabhängigen Prüfer benötigt. Auf Windows kommen MSVC Build Tools hinzu, zum Bauen des Installers außerdem NSIS. Der Drucker bleibt ein eigenes .NET-Projekt.

```sh
cargo run -p druckwandel-native --locked
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/fetch-validators.py
cargo test --locked --test external_validators -- --ignored
```

Das Prüfpaket muss auf dem Zielbetriebssystem geladen werden: Es enthält eine plattformspezifische Java-Laufzeit und wird offline ausgeführt. [Werkzeuge und Lizenzen](resources/validators/README.md).

```sh
cargo run -p druckwandel-native --bin druckwandel-recognize -- quelle.pdf
cargo run -p druckwandel-native --bin druckwandel-xml -- entwurf.json rechnung.xml
cargo run -p druckwandel-native --bin druckwandel-zugferd -- entwurf.json quelle.pdf rechnung.pdf
```

## Installer und CI

[GitHub Actions](https://github.com/Lauorez/druckwandel/actions/workflows/native-slint.yml) prüft Rust, Schemaerkennung und beide Ausgabeformate unter Windows und macOS. Ein manueller Workflow-Lauf mit `package=true` baut zusätzlich die Installer als Artefakte. Unter Windows wird dafür das vorhandene signierte Druckerpaket aus dem bisherigen Release wiederverwendet.

Lokal:

```powershell
python scripts/fetch-validators.py
./scripts/build-windows-installer.ps1 -PrinterPackagePath <Printer.msix> -PrinterDependencyPath <WindowsAppRuntime.msix> -PrinterCertificatePath <PrinterCertificate.cer>
```

```sh
python3 scripts/fetch-validators.py
sh scripts/build-macos-installer.sh
```

Der Windows-Installer registriert `erechnung-review://print-job/<UUID>` für die Slint-EXE. Eine zweite gestartete Instanz übergibt ihre Argumente an die laufende App. Die Deinstallation entfernt nur die eigene Protokollzuordnung und die installierten Dateien; lokale Entwürfe bleiben erhalten. Einrichtung und Entfernung des Druckers benötigen Administratorrechte im selben Benutzerkonto. Unsignierte Entwicklungsinstaller sind keine Produktionsfreigabe.

## Verzeichnisse

| Verzeichnis | Inhalt |
| --- | --- |
| `apps/native/` | Rust-Fachkern, Slint-Oberfläche, Tests |
| `resources/validators/` | Manifest und lokale Java-Prüfer |
| `installer/windows/` | Eigenständiges NSIS-Setup, Druckerinstallation |
| `printer/` | Windows-Print-Support-Komponente in .NET NativeAOT |
| `docs/` | Produktplan und technische Verträge |

Der Projektquellcode steht unter MIT. Slint und weitere Komponenten haben eigene Lizenzbedingungen; siehe [Drittanbieterhinweise](THIRD-PARTY-NOTICES.md).
