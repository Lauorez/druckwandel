# Code-Audit vom 11.11.2026

Ausgangsstand: `main`, `e5be03e`. Durchführung lokal auf macOS mit Rust 1.92.0 und .NET 10.0.401. Die Korrekturen liegen im lokalen Arbeitsverzeichnis; dieser Bericht ist keine Windows- oder Produktionsfreigabe.

## Plan und Prüfumfang

1. Eingänge und Vertrauensgrenzen erfassen: PDF-Dateien, JSON-Entwürfe, Druckauftrag-Metadaten, Protokollaufrufe und Erkennungsanbieter.
2. Datenfluss von Import und Entwurf über Formular und Berechnung bis XML/PDF, unabhängige Prüfung, Dateiersetzung und Mailfreigabe verfolgen.
3. Speicherfehler, unbeabsichtigte Datenänderungen, Betragsberechnung, Parsergrenzen und Prozessaufrufe im Code prüfen.
4. PrintCore, Windows-Installer, macOS-Paketierung, Prüferdownloads und CI statisch einbeziehen.
5. Reproduzierbare Probleme beheben und Regressionen hinzufügen; anschließend Tests, unabhängige Prüfer und Abhängigkeitsscan ausführen.

Geprüft wurden die Rust-Module unter `apps/native/src/`, die Slint-Oberfläche, Rust-Integrationstests, PrintCore und die Übergabe in der Windows-Companion-App sowie Installer-, Download- und Buildskripte. Der eingefrorene Legacy-Code und der separate Dependabot-Branch wurden nicht geändert.

## Behobene Befunde

Die Priorität bewertet die Auswirkung im Projekt. Advisory-Schweregrade werden dadurch nicht ersetzt.

| ID | Priorität | Befund und Korrektur | Nachweis |
| --- | --- | --- | --- |
| A01 | Hoch | Erneute Erkennung ersetzte Positionen allein wegen leerer Bezeichnungen, auch bei manuell bearbeiteten Mengen, Preisen oder Steuerdaten. Ersetzt werden jetzt nur eine leere Liste oder die unveränderte Standardposition. | `tests/recognition.rs`: vier Varianten unbenannter, bearbeiteter Positionen sowie Standardplatzhalter. |
| A02 | Hoch | Beim ZUGFeRD-Export wurde die Original-PDF ohne erneuten Hashvergleich gelesen. Auch die Textanzeige prüfte den Import-Hash nicht. Beide verwenden nun begrenzte, verifizierte Dateibytes; der GUI-Export prüft den Beleg auch bei XML-Ausgabe erneut. | `file_io.rs`: Veränderung bei gleicher Dateigröße wird abgelehnt. |
| A03 | Mittel | Formular-Roundtrips änderten bestehende elektronische Adressschemata auf `0204` oder `EM` und erzeugten leere Verkäuferkontakte. Unveränderte Schemas und fehlende Kontakte bleiben erhalten. | Headless-Formular-Roundtrip und Endpoint-Test in `main.rs`. |
| A04 | Mittel | UBL ließ die Steuernummer weg; CII ließ die Zahlungsreferenz weg. Beide Formate verloren die zweite Lieferadresszeile. Diese Angaben werden jetzt ausgegeben und XML-escaped. | `tests/audit_exports.rs`; zusätzliche KoSIT-/Mustang-Prüfung in `tests/external_validators.rs`. |
| A05 | Hoch | Steuergruppen verglichen auf zwei Stellen gerundete Steuersätze; verschiedene Sätze konnten dadurch zusammenfallen. CII rundete zusätzlich den ausgegebenen Gruppensatz. Verglichen werden jetzt exakte Dezimalwerte; CII erhält die benötigte Genauigkeit. | Zwei Sätze von 19.001 und 19.004 Prozent auf je 10.000 ergeben getrennte Gruppen und 3.800,50 Steuer. |
| A06 | Mittel | Zahlungen aus Ursprungsrechnungen konnten negativ oder feiner als Cent sein; Einzelrundungen konnten inkonsistente Summen erzeugen. Diese Werte werden nun vor Export und bei Berechnung abgelehnt. Datumsangaben der referenzierten Rechnungen und doppelte Positionsnummern werden ebenfalls geprüft. | `tests/audit_exports.rs`: negative, ungültige und Subcent-Beträge, ungültiges Referenzdatum, doppelte IDs. |
| A07 | Hoch | Bei fehlendem Steuernachweis verwendete die Tabellenerkennung automatisch 19 Prozent. Bei mehreren Steuersätzen wurde der erste globale Satz auf weitere Positionen übertragen. Ohne eindeutigen oder positionsbezogenen Nachweis bleibt das Feld nun leer, mit Warnung und reduzierter Konfidenz. | Synthetische Tabellen mit fehlenden beziehungsweise mehreren globalen Steuersätzen. |
| A08 | Mittel | Bei mehreren Datumsfeldern in derselben Zeile nahm die Erkennung jeweils das erste Datum. Sie sucht jetzt hinter dem passenden Feldlabel. | Rechnungs- und Fälligkeitsdatum in einer gemeinsamen Zeile. |
| A09 | Mittel | Dokument und Druckauftrag-Zuordnung wurden getrennt committed. Ein Abbruch dazwischen konnte beim erneuten Öffnen ein zweites Dokument erzeugen. Beide Datenbankeinträge liegen nun in einer Transaktion; Fehler entfernen die neue PDF. | SQLite-Trigger simuliert Fehler beim zweiten Insert: kein Dokument, kein Mapping, keine PDF bleibt zurück. |
| A10 | Mittel | Ein relativer Ausgabename ohne Verzeichnis, wie in den dokumentierten CLI-Aufrufen, wurde wegen des leeren Elternpfads abgelehnt. Er verwendet nun das Arbeitsverzeichnis. | `output.rs`: relative Ausgabe, Ersetzung vorhandener Datei und Hashprüfung bei gleich großer Manipulation. |
| A11 | Hoch | Die ZUGFeRD-CLI erlaubte die Original-PDF als Ausgabeziel. Der Pfadschutz wird nun von GUI und CLI verwendet und erkennt auch kanonische Pfad-Aliase. | `output.rs`: gleicher Pfad über `./` wird abgelehnt. |
| A12 | Mittel | Mehrere Eingänge wurden vollständig gelesen, bevor Größenlimits geprüft wurden; die Erkennungs-CLI las Hash und Text zudem aus getrennten Dateilesungen. Gemeinsame begrenzte Reads prüfen reguläre Dateien und lesen höchstens Limit plus ein Byte; Hash und Extraktion verwenden denselben Puffer. | `file_io.rs`: zu große Datei und Verzeichnis werden abgelehnt; Integration aller drei CLI-Einstiege. |
| A13 | Mittel | Ein fehlgeschlagener oder falsch gehashter Prüferdownload überschrieb bereits die funktionierende JAR. Downloads ersetzen das Ziel jetzt erst nach erfolgreichem Download und Hashvergleich. | Drei Offline-Python-Tests: Prüfsummenfehler, Abbruch und erfolgreiche Ersetzung; auch in CI eingebunden. |
| A14 | Mittel | KoSIT-Meldungen wurden nur als rohe SVRL-Assertions gelesen, obwohl der Prüfer sie in `message`-Elemente umwandelt. Die konkreten Fehler erscheinen nun auch aus diesem Berichtsformat. | Unit-Test mit transformiertem Bericht; zusätzliche Tests für Ablehnung fehlender, negativer oder falscher Mustang-/veraPDF-Berichte. |
| A15 | Hoch/Mittel | Gepinnte oder transitive Abhängigkeiten enthielten bekannte Sicherheitsprobleme. Die betroffenen Versionen wurden gezielt aktualisiert; Details unten. | Erneuter OSV-Abgleich und Subprozess-Test mit tief verschachtelter PDF. |

Der E-Mail-Callback prüft zusätzlich selbst, ob die Ausgabe noch freigegeben und kein Export aktiv ist. Die vorhandene Byteprüfung der gespeicherten Datei bleibt bestehen.

Die eingefrorenen CII-Referenzdateien wurden nicht verändert. Der Vergleich mit dem Legacy-Export lässt gezielt die dort fehlende Zahlungsreferenz aus; ein eigener Test prüft ihre korrekte Ausgabe in beiden Formaten.

## Abhängigkeiten

Der initiale OSV-Abgleich des Cargo-Lockfiles wurde anhand der exakten Paketnamen und Versionen durchgeführt. Meldungen mit Alias-IDs wurden als dasselbe Problem eingeordnet; Wartungshinweise wurden von Sicherheitsproblemen getrennt.

Gezielte Aktualisierungen:

- `lopdf` auf 0.42.0 und kompatibel dazu `pdf-extract` auf 0.12.1. [RUSTSEC-2026-0187](https://rustsec.org/advisories/RUSTSEC-2026-0187.html) beschreibt einen Stackoverflow beim Laden tief verschachtelter PDF-Objekte. Da dieser Prozessabbruch nicht mit `catch_unwind` abgefangen wird, prüft `tests/hostile_pdf.rs` eine PDF mit 10.500 Verschachtelungsebenen in der Erkennungs-CLI als Subprozess auf normalen Fehlerabschluss.
- `rusqlite` auf 0.41.1, dadurch gebündeltes `libsqlite3-sys` auf 0.39.1. [RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459.html) betrifft Speicherpreisgabe bei präparierten Datenbank-/WAL-Dateipaaren. Druckwandel importiert keine fremden Datenbanken, verwendet aber den betroffenen gebündelten SQLite-Code; die Aktualisierung beseitigt diese Abhängigkeit.
- `pulldown-cmark` auf 0.13.6. Die Meldungen [RUSTSEC-2026-0414](https://rustsec.org/advisories/RUSTSEC-2026-0414.html) und [RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433.html) betreffen Ressourcenverbrauch bei bestimmten Wikilinks. Die Abhängigkeit kommt über Slint; es wurde kein eigener Import fremder Markdown-Dateien nachgewiesen.
- `rustix` auf 1.1.6 für [RUSTSEC-2026-0486](https://rustsec.org/advisories/RUSTSEC-2026-0486.html), einen Fehler beim Umgang mit leeren Extended-Attribute-Puffern. Keine direkte Verwendung dieser Funktionen im Anwendungscode gefunden.

Der abschließende OSV-Abgleich umfasst 713 Registry-Paketversionen im Lockfile. Übrig bleiben acht reine Hinweise auf eingestellte Pflege: `bincode`, `paste`, `pin-utils`, `rustybuzz`, `ttf-parser`, `zbus_names`, `zvariant` und `zvariant_derive`. Für diese meldet die Datenbank keine korrigierte Version. Sie benötigen eine gesonderte Bewertung beziehungsweise Migration der jeweiligen übergeordneten Bibliotheken, keine ungeprüften lokalen Ersetzungen. Das Lockfile enthält auch Pakete anderer Zielplattformen; ein Treffer allein belegt keinen ausführbaren Angriffspfad.

Die NuGet-Prüfung mit `--vulnerable --include-transitive` meldet für PrintCore und dessen Tests keine bekannten verwundbaren Pakete. Windows-spezifische AppSDK-/NativeAOT-Projekte sind dadurch nicht vollständig abgedeckt. Java-Prüfer wurden in diesem Audit funktional geprüft, aber nicht gegen eine vollständige Java-Advisory-Datenbank abgeglichen.

Die maschinenlesbaren lokalen Scan-Snapshots liegen unter `artifacts/audit/` und sind wie andere Buildartefakte nicht versioniert.

## Verifikation

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --locked --test external_validators --test kosit -- --ignored
python3 -m unittest discover -s scripts -p 'test_*.py'
cd printer
dotnet test tests/PrintCore.Tests/PrintCore.Tests.csproj --configuration Release
dotnet list tests/PrintCore.Tests/PrintCore.Tests.csproj package --vulnerable --include-transitive --format json
```

Ergebnis: 36 reguläre Rust-Tests, vier separat ausgeführte Prüfertests, zehn .NET-Tests und drei Python-Tests bestanden. Die externen Tests prüfen alle elf vorhandenen Exportfälle in UBL und CII sowie ZUGFeRD einschließlich PDF/A-3B und bytegleichem XML-Anhang. Formatierung und Clippy sind fehlerfrei.

## Grenzen und verbleibende Arbeiten

- Windows-Drucksystem, NSIS-Installation, Update und Deinstallation wurden hier statisch geprüft, nicht auf Windows ausgeführt. Auch die echte Mailfreigabe und signierte Installer benötigen weiterhin Betriebssystemtests.
- Eingabegrößenlimits und der Parserfix ersetzen keine Prozessisolation oder vollständiges PDF-Fuzzing. Für PDF-Dekompression, Fontparser und Rendering ist kein umfassendes CPU-/RAM-Limit nachgewiesen. Die Erkennung und Teile der PDF-Analyse laufen weiterhin auf dem UI-Thread und können bei komplexen Belegen die Bedienung verzögern.
- JARs und KoSIT-Konfiguration sind per Manifest-Hash festgelegt. Die Java-Laufzeit wird weiterhin über einen beweglichen Adoptium-Download bezogen; vollständig reproduzierbare Prüfpakete benötigen eine festgelegte Runtime-Version mit geprüftem Archivhash. Die gesamte Aktualisierung mehrerer Prüferverzeichnisse ist nicht als ein atomarer Paketwechsel implementiert.
- Wartungshinweise für transitive Bibliotheken bleiben als technische Folgearbeit sichtbar. Der Audit ist eine gezielte Code- und Regressionstestprüfung, keine Zusage, dass keine weiteren Fehler existieren.
- Die aktuellen Code- und Lockfile-Änderungen wurden nicht gepusht. Der bestehende Cargo-Dependabot-PR muss vor einer späteren Übernahme gegen diesen Stand abgeglichen werden.
