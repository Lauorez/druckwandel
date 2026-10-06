# WP11 – Rechnungsumfang

Stand: 11.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## WP11a – Gutschrift, Korrektur und Zu-/Abschläge

- Belegart ist ausdrücklich wählbar: Rechnung (380), Gutschrift (381), Rechnungskorrektur (384). Gutschrift und Korrektur brauchen die Nummer der Ursprungsrechnung; Beträge bleiben positiv.
- Positions- und belegweite Nachlässe/Zuschläge gehören zum Rechnungsmodell. Steuergruppen, Rundung und Summen (Positionssumme, Netto, Steuer, Zahlbetrag) werden daraus berechnet.
- „Gutschrift“ und „Rabatt“ als Wort allein sperren die Erstellung nicht mehr. Erkannte Gutschriften und ausgewiesene Rabattzeilen werden in den Entwurf übernommen und müssen bestätigt werden.
- XRechnung erzeugt für 381 ein UBL-CreditNote mit BillingReference; ZUGFeRD/CII setzt TypeCode, InvoiceReferencedDocument und SpecifiedTradeAllowanceCharge. Alte Entwürfe ohne die neuen Felder bleiben lesbar.
- DATEV bleibt für Gutschrift/Korrektur gesperrt. Normale Rechnungen mit bestätigtem Nachlass mindern die Erlöse desselben Steuersatzes; keine erfundenen Ausgleichsbuchungen.

## WP11b – Steuerfreie Fälle und Reverse Charge

- Steuerfälle sind ausdrücklich wählbar: 19 %, 7 %, Reverse Charge (§ 13b UStG / AE), Steuerfrei (§ 4 UStG / E), innergemeinschaftliche Lieferung (K) und 0 % steuerbar (Z). 0 % allein legt keine Kategorie fest.
- Reverse Charge und innergemeinschaftliche Lieferung brauchen Umsatzsteuer-IDs von Absender und Empfänger sowie einen Befreiungsgrund; bei Steuerfreiheit muss der Grund selbst eingetragen werden, ohne geratenen Paragraphen.
- Erkannter Reverse-Charge- oder Steuerfrei-Wortlaut sperrt die Erstellung nicht mehr. 0-%-Positionen übernehmen den erkannten Fall, 19-%-Positionen bleiben Standardsteuer. Alte Entwürfe mit 19/7 bleiben lesbar; nackte 0 % bleiben leer, bis der Fall gewählt ist.
- UBL/CII schreiben Befreiungsgrund und -code (`VATEX-EU-AE` / `VATEX-EU-IC`) in die Steuergruppe, nicht in die Positionssteuer. Innergemeinschaftliche Lieferungen benötigen Leistungsdatum und Lieferland; bei XRechnung werden zusätzlich Lieferort und Postleitzahl geprüft. Lieferangaben werden ausdrücklich erfasst.
- DATEV bleibt für AE, E, K und Z gesperrt. Export und Ausfuhr (G/O) sind nicht Teil von WP11b.

## WP11c – Abschlag, Anzahlung und Schlussrechnung

- Belegart ist ausdrücklich wählbar: Abschlagsrechnung (326), Anzahlungsrechnung (380 mit Kennzeichen, weil XRechnung den Code 386 nicht zulässt) und Schlussrechnung (380 mit bisherigen Belegen). Der Zahlbetrag ist der Rechnungsbetrag abzüglich der bereits gezahlten Beträge; das ist kein Rabatt.
- Schlussrechnungen und Rechnungen mit bereits gezahlten Beträgen brauchen die Nummern der bisherigen Abschläge oder Anzahlungen. Mehrere Vorausrechnungen mit Beträgen werden aufaddiert und als PrepaidAmount / TotalPrepaidAmount ausgegeben.
- Erkannter Wortlaut wie „Abschlagsrechnung“, „Anzahlungsrechnung“ oder „Schlussrechnung“ sperrt die Erstellung nicht mehr. Erkannte Vorausrechnungen und bereits gezahlte Beträge werden in den Entwurf übernommen und müssen bestätigt werden.
- Alte Entwürfe ohne diese Felder bleiben lesbar. DATEV bleibt für 326, Anzahlungsrechnungen, Schlussrechnungen und bereits gezahlte Beträge gesperrt.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Berechnung, Ursprungsbezug, XML CreditNote/Allowance | `test/domain.test.ts`, `test/engine.test.ts`, `test/review.test.ts` |
| Steuerfälle AE/E/K, keine 0-%-Rate, XML-Befreiungsgrund | `test/domain.test.ts`, `test/engine.test.ts`, `test/review.test.ts` |
| Prepaid, Schlussrechnung, TypeCode 326 und Anzahlung als 380 | `test/domain.test.ts`, `test/engine.test.ts`, `test/review.test.ts`, `npm run check:xml` |
| DATEV-Sperre 381, Nachlass auf 380, Sperre AE/Z, Sperre Abschlag/Schluss | `test/datev.test.ts` |
| Alte Entwürfe, Belegart und Steuerfall in der Prüfansicht | `test/workspace.test.ts`, `test/review-ui.test.tsx` |
| Referenzkorpus | 12/12 Fälle, davon 1 bewusst blockiert (Scan) |

Nicht Teil dieser Abnahme: DATEV-Mapping für AE/E/K/Z, Gutschriften oder Abschlags-/Schlussrechnungen sowie ein echter DATEV-Testimport.

## Bewusste Grenzen

Eine technisch gültige Gutschrift oder Reverse-Charge-Rechnung ist keine Buchungsfreigabe. Negative Positionen bleiben ausgeschlossen; Nachlässe gehören in die Zu-/Abschläge. 0 % ohne gewählten Steuerfall bleibt ungültig.
