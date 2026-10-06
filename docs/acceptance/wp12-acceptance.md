# WP12 – Sicherung, Wiederherstellung und Absturzbehandlung

Stand: 16.09.2026. Lokale Entwicklungsabnahme, keine 1.0-Freigabe.

## Umsetzung

- Eine Anwendungssicherung enthält Archivdatenbank und referenzierte Belege, Arbeitsbestand mit Original-PDFs, ältere JSON-Entwürfe, Vorlagengedächtnis, DATEV-Exporthistorie/-dateien und den Archivschlüssel.
- SQLite-Snapshots entstehen über die Backup-API, nicht durch Kopieren einer laufenden WAL-Datei. Schreibzugriffe auf Archiv, Entwürfe, DATEV-Export und Gedächtnis werden während des Snapshots koordiniert pausiert.
- Das Paket ist eine `.erechnung`-Datei: Inhaltsverzeichnis mit Formatversion, Dateiliste und SHA-256, verschlüsselt mit `age` und einem Kennwort (mindestens 12 Zeichen). Der private Schlüssel liegt darin nur verschlüsselt.
- Unter Windows wird der lokale Schlüssel mit DPAPI im Benutzerkontext geschützt; vorhandene 32-Byte-Dateien werden beim Lesen übernommen.
- Wiederherstellung schreibt zuerst in einen isolierten Staging-Ordner, prüft Kennwort, Pfade, Schema, Dateien und Archivkette und zeigt eine Vorschau. Erst nach Bestätigung wird der aktuelle Bestand unter `replaced\<id>` beiseitegelegt und die Sicherung übernommen. Zwei Archivketten werden nicht zusammengeführt.
- Ein dauerhaftes Vorgangsprotokoll erlaubt das Fortsetzen nach einem Abbruch. Verdächtige oder unvollständige Dateien werden nicht automatisch gelöscht oder an die Hash-Kette angepasst.
- Erinnerung, wenn noch nie oder länger als 14 Tage nicht gesichert wurde. Eine Kopie auf demselben Datenträger wird ausdrücklich nicht als Schutz gegen Plattenausfall dargestellt.

## Prüfungen

| Prüfung | Ergebnis |
| --- | --- |
| Roundtrip in leeres Testprofil: bytegleiche Belege, gültige Archivkette, fortsetzbarer Entwurf, Schlüssel | `backup::tests::roundtrip_restores_archive_draft_and_key` |
| Falsches Kennwort, manipulierte Datei | `backup::tests::rejects_wrong_password_and_tampered_package` |
| Pfadtraversal im Paket | `backup::tests::rejects_path_traversal_in_pack` |
| Bisheriger Bestand bleibt nach Wiederherstellung erhalten | `backup::tests::keeps_previous_data_after_restore` |
| Unterbrochene Wiederherstellung fortsetzen | `backup::tests::resumes_interrupted_restore` |
| DPAPI-Hülle und Migration alter 32-Byte-Schlüssel | `protect::tests` |
| Sicherung in den Einstellungen | `test/settings-ui.test.tsx` |
| WP11c XRechnung/CII inkl. Anzahlung als 380 | `npm run check:xml` (22/22) |

Nicht Teil dieser Abnahme: echter Rechnerwechsel auf einem zweiten Windows-Konto, volle Platte als Hardwarefehler und Zusammenführung zweier unabhängiger Archive.

## Bewusste Grenzen

Eine lokale Sicherung ersetzt kein unveränderbares Langzeitarchiv. Das Kennwort kann die Anwendung nicht wiederherstellen. Ein Rechnerwechsel braucht dasselbe Kennwort; der Windows-DPAPI-Schutz allein gilt nur für denselben Benutzer auf demselben Rechner.
