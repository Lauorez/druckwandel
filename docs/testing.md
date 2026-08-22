# Windows-Testplan

## Smoke Test

1. Windows mit `winver` prüfen: mindestens Windows 11 24H2, Build 26100.
2. Repository klonen und PowerShell im Repository öffnen.
3. `Set-ExecutionPolicy -Scope Process Bypass` ausführen, falls lokale Skripte blockiert werden.
4. `.\scripts\build.ps1` ausführen.
5. `.\scripts\install.ps1` ausführen.
6. `Get-Printer -Name "E-Rechnung"` prüfen.
7. Notepad öffnen, Text eingeben und auf `E-Rechnung` drucken.
8. Bestätigen, dass kein Speichern-unter-Dialog erscheint.
9. In der Companion-App Job-ID, Dokumentname, PDF-Pfad und Seitenzahl prüfen.
10. `Open PDF` und `Open Folder` testen.
11. `.\scripts\uninstall.ps1` ausführen und prüfen, dass die Queue verschwindet.

## Testmatrix

| Test | Quelle | Inhalt | Erwartung |
|---|---|---|---|
| A | Notepad | eine Seite Text | PDF und Companion öffnen |
| B | Word/Notepad | mehrere Seiten | vollständige Seitenzahl |
| C | Word | A4 Hochformat | korrekte Geometrie |
| D | Word/Browser | A4 Querformat | korrekte Rotation |
| E | Browser/Word | Text und Bild | visuell korrekt, Farbe erhalten |
| F | Excel | Tabelle | Umbrüche und Skalierung korrekt |
| G | zwei Anwendungen | zwei schnelle Jobs | zwei UUID-PDFs, richtige Zuordnung |

## Diagnose

Die Companion-App zeigt den lokalen Ordner über `Open Folder`. Darunter liegen:

```text
ERechnung\PrintJobs\<UUID>.pdf
ERechnung\PrintJobs\<UUID>.json
ERechnung\Logs\<UUID>.jsonl
```

Bei fehlender Queue zusätzlich in der Ereignisanzeige prüfen:

```text
Applications and Services Logs
  Microsoft
    Windows
      PrintService
      AppModel-Runtime
      AppXDeployment-Server
```

Für einen Fehlerbericht bitte Windows-Build, Visual-Studio-Version, Buildausgabe, relevante Ereignisanzeige-Einträge und die JSONL-Datei des Jobs sichern. Rechnungsinhalte nicht veröffentlichen.
