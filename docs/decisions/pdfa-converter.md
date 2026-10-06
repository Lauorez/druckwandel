# PDF/A-Konverter (WP9)

Stand: 07.09.2026.

## Kandidaten

| Weg | Lizenz | Weiterverteilung | Eignung |
| --- | --- | --- | --- |
| Ghostscript `pdfwrite`/PDFA | AGPL bzw. kommerziell (Artifex) | ohne AGPL-Gesamtprojekt oder bezahlte Lizenz nicht vertretbar | bewusst nicht gewählt |
| Apache PDFBox direkt | Apache-2.0 | ja | keine fertige, zuverlässige Konvertierung beliebiger Druck-PDFs |
| Mustang CLI (`a3only`/`combine`) | Apache-2.0, bereits gebündelt | ja | setzt typischerweise bereits PDF/A-1 voraus |
| Strukturelle PDF/A-3-Vorbereitung mit pdf-lib plus ICC-OutputIntent | MIT (pdf-lib), ICC als compact RGB-Profil | ja | Seiteninhalt bleibt erhalten; fehlende Schriften, Formulare, Verschlüsselung und beschädigte Dateien werden abgewiesen |
| veraPDF Greenfield 1.28.2 | GPL-3.0-or-later oder MPL-2.0 | MPL-2.0 für die gebündelte Prüfung | unabhängige PDF/A-3b-Prüfung, kein Konverter |

## Entscheidung

1. **Kein Ghostscript.** Die Lizenz erlaubt keine stille Mitlieferung in diesem Produkt.
2. **Konverter:** Die Anwendung erzeugt eine neue PDF/A-3-Datei aus der unveränderten Original-PDF, ohne Neurendern. Sie setzt OutputIntent, Sprache und Factur-X-XMP, bettet dieselben bereits erzeugten CII-Bytes als `factur-x.xml` ein und ändert nach der Prüfung nichts mehr. Schriften müssen bereits eingebettet sein. Formulare, Verschlüsselung und widersprüchliche vorhandene E-Rechnungsanhänge werden blockiert.
3. **Prüfung:** veraPDF 1.28.2 (Flavour `3b`) und Mustang (XML plus Extrakt aus der fertigen PDF) sind verbindlich. Exitcode 0 allein reicht nicht.
4. Es gibt **keine Zusage**, jedes beliebige Druck-PDF umwandeln zu können. In diesem Fall bleibt die XML-Ausgabe nutzbar, die Hybrid-PDF nicht.

Paketgröße: das veraPDF-JAR liegt in der Größenordnung von 9 MB plus der bereits vorhandenen JRE. Laufzeit: dieselbe 120-Sekunden-Grenze wie bei WP8.
