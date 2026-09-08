# Gebündelte XML- und PDF/A-Prüfer (WP8/WP9)

Dieses Verzeichnis enthält das Standardpaket für die lokale, unabhängige Prüfung.
JAR-Dateien, XRechnung-Konfiguration und die private Java-Laufzeit werden nicht ins Git gelegt.

## Inhalt nach `npm run validators:fetch`

- `jre/` – Eclipse Temurin 21 JRE, nur für diese Anwendung
- `kosit/validator-*-standalone.jar` – KoSIT Validator
- `kosit/xrechnung/` – XRechnung-Szenarien und Regelartefakte
- `mustang/Mustang-CLI-*.jar` – Factur-X/ZUGFeRD-XML-Prüfung und XML-Extrakt
- `verapdf/greenfield-apps-*.jar` – PDF/A-3b-Prüfung

Versionen, Herkunft und Lizenzen stehen in `manifest.json`. Beim Umsetzen am 07.09.2026 gegen die Herausgeberseiten geprüft: KoSIT Validator 1.6.3, XRechnung-Konfiguration 3.0.2 (Release 2026-01-31), Mustang-CLI 2.16.2, veraPDF Greenfield 1.28.2, Temurin 21.

Die Anwendung startet ausschließlich diese gebündelten Dateien. Sie verwendet kein Java aus PATH und keinen HTTP-Daemon.

Ohne KoSIT/Mustang entsteht keine fertige E-Rechnung. Ohne veraPDF entsteht keine fertige PDF-Rechnung.
