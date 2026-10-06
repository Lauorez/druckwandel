# WP10 – Messbare Erkennungsqualität und kontrollierbares Lernen

Stand: 10.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- Referenzkorpus um Seitenumbrüche, ähnliche Parteien, wiederholte Werte, eine weitere Layoutfamilie und eine geänderte Fußzeile erweitert. Bestehende sechs Fälle bleiben Regressionstor.
- Lernen und Prüfen sind getrennt: Regeln entstehen auf Rechnung A und werden auf unbekannten B/C geprüft. Gespeicherte Regeln enthalten keine konkreten Rechnungswerte.
- Eine Regel gilt nur bei passendem Layout- oder Vorlagenanker. Dieselbe Firma mit anderer Vorlage wird nicht still befüllt. Absolute Regionen werden nur bei eindeutiger Übereinstimmung angewendet; verschobene Blöcke brauchen einen Textanker.
- Widersprüchliche oder mehrdeutige Treffer ersetzen keinen bereits erkannten Wert. Unsichere gelernte Übernahmen sind im Formular als prüfbedürftig gekennzeichnet.
- Einzelne gemerkte Zuordnungen lassen sich anzeigen, deaktivieren und entfernen. Die letzte Bestätigung im aktuellen Erkennungsprofil ist rückgängig machbar. Alte Regeln ohne Herkunftsangabe bleiben lesbar.
- `evaluate-corpus` schreibt zusätzlich `artifacts/quality-report.json` mit Feld-/Positionsgenauigkeit, Fehlzuordnungen, Blockierungen und nötigen Korrekturen. Keine Telemetrie.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Lernen A→B, unpassende Vorlage, keine alten Werte, Deaktivieren/Undo | `test/learning.test.ts` |
| Gemerkte Zuordnungen in der Review-Oberfläche | `test/learning-ui.test.tsx` |
| Qualitätsbericht | `test/evaluation.test.ts` |
| Referenzkorpus | 11/11 Fälle nach `npm run corpus:generate && npm run corpus:evaluate` |
| Bestehende Unit-Tests | `npm test` |

Synthetische Treffer und Referenzfälle sind keine gemessene Kundenquote. Zusätzliche echte Kundenbelege bleiben lokal unter `test/corpus/private`.

## Bewusste Grenzen

Unbeaufsichtigtes Fertigstellen allein aufgrund gelernter Zuordnungen bleibt ausgeschlossen. Eine hohe Trefferquote im Referenzkorpus ist kein Nachweis für beliebige Kundenlayouts.
