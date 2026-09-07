# WP6: Ground-Truth-Korpus

Der Korpus ist das verpflichtende Quality Gate für Änderungen an PDF-Extraktion, Review-Mapping und Unsupported-Case-Erkennung.

## Standardlauf

    npm run wp6:check

Der Befehl erzeugt sechs deterministische, anonymisierte Referenz-PDFs, vergleicht ihre Extraktion mit test/corpus/manifest.json und führt anschließend 250 generierte Rechnungen durch die komplette PDF-Pipeline. Geprüft werden:

- normalisierte Rechnungsfelder,
- Anzahl und Inhalt der Positionen,
- erwartete Warncodes,
- erwartete blockierende Sonderfälle.

Die Ergebnisse werden zusätzlich maschinenlesbar nach artifacts/corpus-report.json und artifacts/synthetic-fuzz-report.json geschrieben. Schon eine Abweichung setzt einen Exitcode ungleich null. npm run check und jeder Desktop-Release-Build führen dieses Gate automatisch aus.

## Seeded Fuzz-Test

Der Fuzz-Test baut zuerst ein strukturiertes Soll-Modell und rendert daraus erst anschließend das PDF. Die Extraktion wird damit gegen die Quelldaten und nicht gegen eigene Parserannahmen verglichen. Variiert werden unter anderem:

- drei Tabellenlayouts, einschließlich zusammengezogener PDF-Texttokens,
- Schriftgröße und horizontale Positionen,
- deutsche Zahlenformate sowie EUR- und Euro-Zeichen,
- 7 und 19 Prozent Umsatzsteuer,
- Mengen, Einheiten, Preise, Parteien und Datumswerte,
- ein- und mehrseitige Rechnungen mit bis zu 30 Positionen.

Der Standardlauf verwendet die Seeds 1000 bis 1249:

    npm run corpus:fuzz

Anzahl und Start-Seed können überschrieben werden. So lässt sich ein gemeldeter Seed einzeln reproduzieren:

    npm run corpus:fuzz -- --count 1 --seed 1042

Bei einem Fehler bleiben unter artifacts/fuzz-failures drei Dateien zum Seed liegen: das gerenderte PDF, das unabhängige Soll-Modell als source.json und der feldgenaue Vergleich als evaluation.json. Der Seed macht den Fall unabhängig vom Rechner reproduzierbar.

Der Generator ergänzt den festen Referenzkorpus, ersetzt aber keine echten, manuell verifizierten Rechnungen. Generierte PDFs und Soll-Daten teilen weiterhin fachlichen Erzeugungscode und können deshalb dieselbe falsche Annahme enthalten.

## Enthaltene Startfälle

1. Tabelle mit Kopfzeile und zusammengezogenem Beschreibung-Menge-Token,
2. ältere Spaltenrechnung ohne Kopfzeile,
3. Gutschrift,
4. Reverse Charge,
5. belegweiter Rabatt mit Summenabweichung,
6. Scan ohne Textlayer.

Die letzten vier Fälle prüfen nicht, ob sie irgendwie in eine XML-Datei passen, sondern ob die Anwendung ihre derzeit fehlende fachliche Unterstützung eindeutig erkennt und die Ausgabe blockiert.

## Lokale echte Rechnungen

Echte Belege gehören in test/corpus/private und bleiben durch .gitignore lokal. Ein separates Manifest kann so ausgeführt werden:

    npm run corpus:evaluate -- --manifest test/corpus/private/manifest.local.json --report artifacts/private-corpus-report.json

Das lokale Manifest verwendet dasselbe Schema wie test/corpus/manifest.json. Feldwerte werden als kanonische Strings angegeben; Geldwerte verwenden intern einen Punkt als Dezimaltrenner.

Vor einer Weitergabe oder Aufnahme in ein gemeinsames Korpus muss ein Beleg tatsächlich anonymisiert werden. Ein schwarzes Rechteck über sichtbarem Text genügt nicht, weil der ursprüngliche PDF-Textlayer weiterhin Namen, Anschriften oder Bankdaten enthalten kann. Sicher ist eine neu erzeugte synthetische Variante mit demselben Layout oder eine vollständige Entfernung der Originalobjekte samt Metadaten.

## Neuen synthetischen Fall ergänzen

1. PDF-Erzeugung in scripts/generate-corpus.ts ergänzen.
2. Erwartete Werte in test/corpus/manifest.json eintragen.
3. npm run wp6:check ausführen.
4. Bei einer absichtlich nicht unterstützten Rechnung den stabilen Policy-Code in src/policy/unsupported-cases.ts ergänzen.
5. Einen Unit-Test für die neue Regel hinzufügen.

Die Fall-ID und Policy-Codes sind stabile Maschinenkennungen. Beschreibungen und deutsche UI-Texte dürfen verbessert werden, ohne historische Reports unbrauchbar zu machen.
