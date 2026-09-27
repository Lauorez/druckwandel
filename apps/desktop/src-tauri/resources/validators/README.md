# Gebündelte XML- und PDF/A-Prüfer (WP8/WP9)

Dieses Verzeichnis enthält das Standardpaket für die lokale, unabhängige Prüfung.
JAR-Dateien, XRechnung-Konfiguration und die private Java-Laufzeit werden nicht ins Git gelegt.

## Inhalt nach `npm run validators:fetch`

- `jre/` – Eclipse Temurin 21 JRE, nur für diese Anwendung
- `kosit/validator-*-standalone.jar` – KoSIT Validator
- `kosit/xrechnung/` – XRechnung-Szenarien und Regelartefakte
- `mustang/Mustang-CLI-*.jar` – Factur-X/ZUGFeRD-XML-Prüfung und XML-Extrakt
- `verapdf/greenfield-apps-*.jar` – PDF/A-3b-Prüfung

Versionen, Herkunft und Lizenzen stehen in `manifest.json`; das Download-Skript liest seine Versionen aus dieser Datei. Am 11.09.2026 heruntergeladen und ausgeführt: KoSIT Validator 1.6.3, XRechnung-Konfiguration 3.0.2 (Release 2026-01-31), Mustang-CLI 2.26.0, veraPDF Greenfield 1.28.2, Temurin 21. Die SHA-256-Werte der Downloads liegen in `checksums.json` mit relativen Pfaden. Die JRE folgt dem aktuellen Temurin-21-Patchstand beim Download.

veraPDF muss über `-cp <jar> org.verapdf.apps.GreenfieldCliWrapper` gestartet werden. `java -jar` startet bei diesem Artefakt die GUI und ist für die Hintergrundprüfung ungeeignet.

`npm run check:xml` und `npm run check:pdf` prüfen die heruntergeladenen Artefakte mit der gebündelten JRE.

Die Anwendung startet ausschließlich diese gebündelten Dateien. Sie verwendet kein Java aus PATH und keinen HTTP-Daemon.

Ohne KoSIT/Mustang entsteht keine fertige E-Rechnung. Ohne veraPDF entsteht keine fertige PDF-Rechnung.
