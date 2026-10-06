# Codebase-Cleanup und Sanity-Check – 11.09.2026

Geprüfter Arbeitsstand: E-Rechnungs-Assistent 0.3.1 unter Windows, einschließlich TypeScript-Core, React/Tauri-Oberfläche, nativer Rust-Persistenz, XML-/PDF-Export und .NET-Druckbrücke. Bereits vorhandene uncommittete Änderungen wurden weiterverwendet und erhalten. Dieser Bericht beschreibt die zusätzlichen Korrekturen; der gesamte Git-Diff enthält auch vorherige Arbeiten. Es wurde kein Commit erstellt.

## Behobene Fehler

| Bereich | Fehler und Korrektur |
| --- | --- |
| Beträge | Kanonische Werte wie `1.000` und `12.345` wurden als deutsche Tausendergruppen fehlinterpretiert. Anzeige und deutsche Eingabe verwenden jetzt getrennte Parsing-Regeln; fehlerhafte Gruppierungen werden abgewiesen. |
| Rechnungsprüfung | Ungültige Datums-/Dezimalwerte, übermäßige Nachlässe und manipulierte berechnete Summen konnten unzureichend geprüft werden. Validierung prüft Leistungsdatum, Präzision, Positionen, Steuergruppen und unabhängig nachgerechnete Summen; ungültige Eingaben liefern Fehler statt Exceptions. Die Prüfansicht verwendet auch die berechneten Invarianten. |
| DATEV | Bei Verteilung von Zu-/Abschlägen über mindestens drei Erlöskonten wurde nach dem ersten Konto mit dem verbleibenden statt dem ursprünglichen Betrag gerechnet. Die Verteilung verwendet jetzt den ursprünglichen Betrag und weist den Rundungsrest dem letzten Konto zu. |
| Speichern | Einstellungen konnten beim Wechseln/Schließen vor Abschluss eines Schreibvorgangs verloren gehen. Navigation, Escape, Hintergrundklick und natives Schließen warten auf ausstehende Schreibvorgänge; Fehler bleiben sichtbar und wiederholbar. Ein fehlgeschlagenes Löschen erhält den geöffneten Entwurf. Lesefehler beim Vorlagengedächtnis erzeugen keinen leeren Ersatzbestand. |
| Ereignisse/UI | Asynchron registrierte Listener werden auch nach frühem Unmount entfernt. Doppelte native/lokale Ereignisse sind beseitigt. DATEV-Vorschauen werden nach Profiländerungen ungültig; laufende Prüfphasen sind sichtbar. |
| UBL/CII | Korrigiert: UBL-Reihenfolge der Summenelemente, Fälligkeitsdatum in CreditNote, CII-Reihenfolge des Befreiungscodes, Großschreibung der VATEX-Codes und Escaping dynamischer XML-Attribute. Innergemeinschaftliche Lieferungen haben explizite Lieferangaben mit Profilvalidierung und korrekter Serialisierung. |
| Validatoren | Die lokale Schemaauflösung innerhalb der Mustang-JAR war durch JVM-Flags blockiert. Lokale Schemaressourcen sind erlaubt, externe DTDs bleiben gesperrt. KoSIT-VARL-Meldungen werden samt Regelcode/Feld ausgewertet. Rust liest die camelCase-Versionsangaben und den Java-Pfad des Manifests korrekt. |
| PDF/A | veraPDF wird über seinen CLI-Einstieg statt den GUI-Einstieg gestartet. Die PDF-Verarbeitung erzeugt die erforderlichen Trailer-Dateikennungen und erhält bei Änderungen die ursprüngliche Kennung. Der Parser erkennt das reale `profileName`-Berichtsformat und weist andere PDF/A-Profile weiterhin ab. |
| Windows/Rust | Der Archivtest schließt SQLite vor dem Löschen seiner temporären Dateien. Export-Ticket-Abgleich verwendet einen strukturierten Kandidaten statt acht Einzelargumenten; Clippy läuft mit Warnungen als Fehler. |

## Struktur und Qualitätsgates

- Die leichte `HybridPdfError`-Klasse liegt separat, damit die Startoberfläche den PDF-Exportcode nicht vorzeitig lädt. Gemeinsame Base64- und Listener-Helfer ersetzen Duplikate; inkonsistente statische/dynamische Importe sind bereinigt. Der Haupt-JavaScript-Chunk sank von rund 858 auf 415,84 kB; der abschließende Vite-Build hat keine Warnungen ausgegeben.
- `npm run check` umfasst Core, Desktop und UI-Test-Typprüfung, Vitest und WP6. `check:native`, `check:xml` und `check:pdf` machen die ergänzenden Prüfungen reproduzierbar.
- Generierter Windows-Installer-Payload ist in `.gitignore` aufgenommen. Die Hauptfenster-Capability gilt nur für das Hauptfenster; die Einstellungen haben eine eigene Capability.
- Das Download-Skript bezieht Validatorversionen aus dem Manifest, schreibt portable Prüfsummenpfade und begrenzt das Entfernen generierter Verzeichnisse auf deren vorgesehenen Elternordner.
- Vitest ist auf 4.1.11 aktualisiert; das vollständige npm-Audit meldet am Prüftag keine bekannten Schwachstellen. Anlass war das [offizielle Vitest-Advisory GHSA-82fw-gwwq-j7x9](https://github.com/vitest-dev/vitest/security/advisories/GHSA-82fw-gwwq-j7x9) für Entwicklungswerkzeuge.

## Verifikation

| Prüfung | Ergebnis |
| --- | --- |
| `npm run check` | Core-/Desktop-/UI-Test-Typprüfung erfolgreich; 157 Tests in 21 Dateien bestanden |
| Referenzkorpus | 12/12 Fälle, 165/165 Felder, 16/16 Positionen; ein Scan wird erwartungsgemäß blockiert |
| Reproduzierbares Fuzzing | 250/250 Rechnungen, Seeds 1000–1249; 4518/4518 Felder, 1841/1841 Positionen |
| `npm run check:native` | Clippy ohne Warnungen; 35 Rust-Tests bestanden |
| `drucker/scripts/test.ps1` | 10 .NET-Core-Tests bestanden; PrintSmokeSender im Release-Modus gebaut, ohne Warnungen |
| `npm run validators:fetch` | Vollständiges Windows-Prüfpaket inklusive privater JRE erfolgreich heruntergeladen |
| `npm run check:xml` | 16/16 externe Prüfungen: Standardrechnung, Zu-/Abschläge, Gutschrift, Korrektur, Reverse Charge, Steuerfreiheit, innergemeinschaftliche Lieferung und 0 %; jeweils UBL und CII |
| `npm run check:pdf` | Musterrechnung extrahiert, Hybrid-PDF erzeugt, XML auch durch Mustang bytegleich extrahiert; veraPDF PDF/A-3b und Mustang erfolgreich |
| Nativer Release-Build | `npx tauri build --config apps/desktop/src-tauri/tauri.conf.json --no-bundle` erfolgreich; Desktop-Typprüfung und Frontend-Build enthalten; EXE unter `apps/desktop/src-tauri/target/release/erechnung-desktop.exe` |
| `npm audit` | 0 bekannte Schwachstellen, einschließlich Entwicklungsabhängigkeiten |
| `git diff --check` | Keine Whitespace-Fehler |

Die XML-/PDF-Prüfungen verwenden die tatsächlich heruntergeladenen KoSIT-1.6.3-, Mustang-2.26.0- und veraPDF-1.28.2-Artefakte mit Temurin 21 und XRechnung-Konfiguration 2026-01-31. Versionen und Quellen stehen im [Validator-Manifest](../../apps/desktop/src-tauri/resources/validators/manifest.json). KoSIT-Release und Konfiguration wurden zusätzlich direkt über die GitHub-Release-API verifiziert; ältere Suchmaschinenstände wurden nicht als Versionsnachweis verwendet.

Lokale maschinenlesbare Ergebnisse liegen in `artifacts/corpus-report.json`, `artifacts/synthetic-fuzz-report.json`, `artifacts/dependency-audit.json`, `artifacts/xml-regressions/report.json` und `artifacts/hybrid-regression/report.json`. Generierte Binärdateien und Prüfartefakte bleiben außerhalb von Git.

## Verbleibende Abnahmegrenzen

- Kein neuer Installer wurde installiert oder signiert; kein tatsächlicher Druckauftrag wurde gesendet. Der gebaute Smoke-Sender und die .NET-Tests ersetzen diesen Integrationstest nicht.
- Kein Import in ein echtes DATEV-System. Die fachlich noch gesperrten Beleg-/Steuerfälle bleiben gesperrt.
- Die PDF/A-Prüfung belegt die erzeugte Musterrechnung. Beliebige Kunden-PDFs benötigen weiterhin die unabhängige Einzelprüfung; nicht eingebettete Schriften, Verschlüsselung und Formulare werden gezielt abgewiesen.
- Referenzkorpus und synthetische Rechnungen messen Regressionen im vorhandenen Umfang, keine allgemeine Erkennungsquote für Kundenbelege. OCR über eine echte Tesseract-Installation wurde in diesem Cleanup nicht separat abgenommen.
- Die Oberfläche wurde durch Komponenten- und Persistenztests geprüft; eine vollständige manuelle Bedienungsabnahme der installierten Anwendung steht separat an.
