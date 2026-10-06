# WP9 – Konforme Hybrid-PDFs und konsistente Rechnungsinhalte

Stand: 07.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- Original-PDF bleibt unverändert. Für die PDF-Rechnung entsteht eine neue PDF/A-3-Datei; dieselben CII-Bytes werden als `factur-x.xml` eingebettet. Danach keine weiteren Byte-Änderungen.
- Vorhandene E-Rechnungsanhänge werden nicht still überschrieben oder verdoppelt. Widersprüchliche Anhänge, Formulare, Verschlüsselung und fehlende Schriften blockieren.
- Summen und belegbare Angaben werden mit der Quellrechnung verglichen. Unaufgelöste Abweichungen verhindern die Fertigstellung. Ergänzte Werte brauchen eine ausdrückliche Bestätigung.
- Native Fertigstellung einer PDF-Rechnung prüft XML (Mustang), den bytegleichen XML-Extrakt aus der PDF und PDF/A-3b (veraPDF). Ohne maschinenlesbaren Bericht gilt die Datei als nicht geprüft.

Die Konverterentscheidung steht in [pdfa-converter-decision.md](../decisions/pdfa-converter.md).

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Einbettung, XML-Identität, widersprüchliche Anhänge, unembedded fonts | `test/hybrid-pdf.test.ts` |
| Inhaltsabgleich und Bestätigung | `test/consistency.test.ts` |
| veraPDF-Berichtparser | `test/validators.test.ts` und `cargo test` |
| Bestehende Format- und Archivtests | `npm test`, `cargo test` |

Echte veraPDF-/Mustang-Läufe gegen fertige Hybrid-PDFs erfordern das Paket aus `npm run validators:fetch` (einschließlich `verapdf/greenfield-apps-*.jar`). Ohne veraPDF lehnt die native PDF-Rechnung ab; XRechnung bleibt davon unabhängig nutzbar, sofern KoSIT vorhanden ist.

## Bewusste Grenzen

Nicht jedes PDF wird zu PDF/A-3. Es gibt keinen stillen Rechnungsgenerator und kein Neurendern bereits ausgestellter Belege. Eine technische Prüfung ist keine sachliche oder steuerliche Richtigkeitsgarantie.
