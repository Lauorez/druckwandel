# Gebündelte Komponenten

Stand: Anwendung 0.3.3, Drucker 0.1.0.12. Keine 1.0-Freigabe. Diese Liste beschreibt die mitgelieferten oder fest eingebundenen Bestandteile, nicht den gesamten transitiven Abhängigkeitsbaum.

SHA-256 der heruntergeladenen Prüfer: `apps/desktop/src-tauri/resources/validators/checksums.json`.

| Komponente | Version | Rolle | Lizenz | Herkunft |
| --- | --- | --- | --- | --- |
| E-Rechnungs-Assistent | 0.3.3 | Anwendung | MIT | dieses Repository |
| Tauri | 2 | Desktop-Laufzeit | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| rusqlite | 0.37 | Archiv/Entwürfe | MIT | https://github.com/rusqlite/rusqlite |
| age | 0.11 | Sicherung | MIT OR Apache-2.0 | https://github.com/str4d/rage |
| ed25519-dalek | 2 | Archivbestätigung | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek |
| Eclipse Temurin | Temurin 21 | Prüflaufzeit | GPL-2.0-with-Classpath-exception | https://adoptium.net/ |
| KoSIT Validator | 1.6.3 | XRechnung | Apache-2.0 | https://github.com/itplr-kosit/validator/releases/tag/v1.6.3 |
| XRechnung-Konfiguration | xrechnung-3.0.2-2026-01-31 | XRechnung-Regeln | Apache-2.0 | https://github.com/itplr-kosit/validator-configuration-xrechnung |
| Mustang-CLI | 2.26.0 | ZUGFeRD-XML | Apache-2.0 | https://repo.maven.apache.org/maven2/org/mustangproject/Mustang-CLI/2.26.0/ |
| veraPDF Greenfield | 1.28.2 | PDF/A-3b | GPL-3.0-or-later OR MPL-2.0 | https://repo1.maven.org/maven2/org/verapdf/apps/greenfield-apps/1.28.2/ |
| E-Rechnungsdrucker | 0.1.0.12 | Druckannahme | MIT | drucker/ |

npm-Laufzeitabhängigkeiten: @napi-rs/canvas, @tauri-apps/api, @tauri-apps/plugin-deep-link, decimal.js, pdf-lib, pdfjs-dist, react, react-dom.

Eine maschinenlesbare Fassung schreibt `node scripts/inventory-components.mjs` nach `artifacts/components.json`. Schwachstellenprüfungen gehören zum Release-Gate; blockierende Befunde verhindern die Auslieferung.
