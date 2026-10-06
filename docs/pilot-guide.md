# Kurzanleitung für den Pilotbetrieb

Stand: 16.09.2026. Version 0.3.1. Keine 1.0-Freigabe.

Die Anwendung arbeitet nur auf diesem Computer. Rechnungsinhalte werden nicht in eine Cloud gesendet.

## Erste Schritte

1. Setup mit dem Konto starten, das die Anwendung nutzt, und die Administratorabfrage bestätigen.
2. In den Einstellungen unter **Sicherung** Archiv und Entwürfe auf ein **anderes Laufwerk** sichern. Eine Kopie auf derselben Platte schützt nicht vor einem Plattenausfall.
3. In Ihrem Rechnungsprogramm den Drucker **E-Rechnung** wählen. Die Anwendung darf geschlossen sein; sie öffnet sich mit dem neuen Beleg.
4. Angaben prüfen, fehlende Felder ergänzen oder im PDF markieren. Erst nach der Bestätigung „Angaben geben die Originalrechnung korrekt wieder“ speichern Sie die fertige E-Rechnung.
5. **Für Behörden speichern** erzeugt die XML-Datei (XRechnung). **Als PDF-Rechnung speichern** erzeugt eine PDF/A-3 mit eingebetteter ZUGFeRD-Datei.

## Unterstützte Rechnungsfälle

- Normale Ausgangsrechnung (380), Gutschrift (381), Rechnungskorrektur (384)
- Abschlagsrechnung (326), Anzahlungsrechnung (380 mit Kennzeichen), Schlussrechnung
- Steuer 19 %, 7 %, Reverse Charge, steuerfrei, innergemeinschaftliche Lieferung, 0 % steuerbar

Nicht unterstützt: Export/Ausfuhr, Istversteuerung, Peppol, DATEV-Online, unbeaufsichtigtes Fertigstellen allein aus gelernten Zuordnungen.

## DATEV (nur Testversion)

Der DATEV-Export gilt nur für bestätigte normale EUR-Ausgangsrechnungen mit 7/19 % und Sollversteuerung. Gutschriften, Korrekturen, Abschläge, Anzahlungen, Schlussrechnungen und die übrigen Steuerfälle bleiben gesperrt. Zuerst `Belege.zip` über DATEV Belegtransfer importieren (nicht entpacken), danach die EXTF-Dateien. Der Importstatus in DATEV ist dieser Anwendung unbekannt. Ein echter Testimport durch die Steuerkanzlei ist Voraussetzung vor produktivem Einsatz.

## Wenn etwas schiefgeht

- Fehlgeschlagenes Update: `%LOCALAPPDATA%\de.erechnung.converter\update-backup\<Version>\WIEDERHERSTELLUNG.txt`
- Unterbrochene Wiederherstellung: Einstellungen → Sicherung → fortsetzen
- Diagnose: Einstellungen → Diagnose. Vorschau prüfen, dann bewusst als JSON speichern. Der Bericht enthält **keine** PDF-, XML-, Bank- oder Rechnungsinhalte und keine persönlichen Dateipfade. Er wird nicht automatisch versendet.

Rückmeldungen bitte mit App-Version, Windows-Build, Testdaten-ID und dem Diagnosebericht; keine Originalrechnungen mitschicken, sofern nicht ausdrücklich vereinbart.
