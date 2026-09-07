# Gebündelte XML-Prüfer (WP8)

Dieses Verzeichnis enthält das Standardpaket für die lokale, unabhängige Prüfung.
JAR-Dateien, XRechnung-Konfiguration und die private Java-Laufzeit werden nicht ins Git gelegt.

## Inhalt nach `npm run validators:fetch`

- `jre/` – Eclipse Temurin 21 JRE, nur für diese Anwendung
- `kosit/validator-*-standalone.jar` – KoSIT Validator
- `kosit/xrechnung/` – XRechnung-Szenarien und Regelartefakte
- `mustang/Mustang-CLI-*.jar` – Factur-X/ZUGFeRD-Prüfung

Versionen, Herkunft und Lizenzen stehen in `manifest.json`. Beim Umsetzen am 07.09.2026 gegen die Herausgeberseiten geprüft: KoSIT Validator 1.5.0, XRechnung-Konfiguration 3.0.2 (Release 2026-01-31), Mustang-CLI 2.16.2, Temurin 21.

Die Anwendung startet ausschließlich diese gebündelten Dateien. Sie verwendet kein Java aus PATH und keinen HTTP-Daemon.

Ohne dieses Paket erzeugt die native App keine fertige E-Rechnung.
