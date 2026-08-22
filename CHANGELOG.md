# Changelog

Alle nennenswerten Änderungen werden in dieser Datei dokumentiert. Die Versionierung folgt [Semantic Versioning](https://semver.org/).

## [0.1.0-beta.1] - 2026-08-23

Erste private Beta für den interaktiven Windows-11-End-to-End-Test.

### Enthalten

- moderner, treiberloser Print Support Virtual Printer `E-Rechnung`,
- OXPS-zu-PDF-Konvertierung über den Windows-`PrintWorkflowPdlConverter`,
- PDF-Passthrough für kompatible Druckquellen,
- UUID-basierte PDF-/JSON-Ablage und atomarer Session-Index,
- lokales JSONL-Statuslogging ohne Telemetrie,
- native WinUI-3-Companion-App mit PDF-/Ordner-Aktionen,
- signiertes x64-MSIX mit Entwicklungszertifikat,
- Build-, Installations-, Deinstallations-, Smoke-Test- und Diagnoseskripte,
- acht Unit-Tests sowie Windows-CI für Paketinhalt und Queue-Lifecycle.

### Vor Veröffentlichung bestätigt

- Release-Build und MSIX-Signatur,
- erforderliche Manifest- und Runtime-Dateien im Paket,
- Installation auf Windows Build 26100,
- automatische Registrierung der Queue `E-Rechnung`,
- Deinstallation und Entfernen der Queue,
- keine bekannten verwundbaren direkten oder transitiven NuGet-Pakete.

### Noch im Betatest zu bestätigen

- interaktiver Druck aus Notepad, Browser, Word und Excel,
- kein zusätzlicher Speichern-unter-Dialog,
- OXPS-zu-PDF-Ausgabequalität und PrintTicket-Übernahme,
- `PrintSupportJobUI`-Aktivierung und Companion-Übergabe,
- mehrere nahezu gleichzeitige Druckjobs,
- ARM64-Paket und Windows on ARM.
