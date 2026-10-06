# Windows-Testplan

## Smoke Test

1. Windows mit `winver` prüfen: mindestens Windows 11 24H2, Build 26100.
2. Repository klonen und PowerShell **als Administrator** im Repository öffnen.
3. `Set-ExecutionPolicy -Scope Process Bypass` ausführen, falls lokale Skripte blockiert werden.
4. `.\scripts\build.ps1` ausführen.
5. `.\scripts\install.ps1` ausführen.
6. `Get-Printer -Name "E-Rechnung"` prüfen.
7. Notepad öffnen, Text eingeben und auf `E-Rechnung` drucken.
8. Bestätigen, dass kein Speichern-unter-Dialog erscheint.
9. In der Companion-App Job-ID, Dokumentname, PDF-Pfad und Seitenzahl prüfen.
10. `Open PDF` und `Open Folder` testen.
11. `.\scripts\uninstall.ps1` ausführen und prüfen, dass die Queue verschwindet.

## Automatischer Desktop-Smoke-Test

In einer interaktiven Windows-Sitzung führt folgender Befehl Installation, A4-XPS-Testdruck, PDF-/JSON-/Statusprüfung und Deinstallation automatisch aus:

```powershell
.\scripts\smoke-install.ps1 -TestPrint
```

Für die vollständige automatisierte Adapter-Matrix:

```powershell
.\scripts\smoke-install.ps1 -TestMatrix
```

Diese Variante prüft zusätzlich Mehrseitigkeit, A4-Querformat, Grafik/Farbe/Tabelle und zwei gleichzeitig gesendete Jobs. Die Seitenzahl wird aus den vom Background-Task erzeugten Jobmetadaten geprüft.

Der Test verändert keine bereits vorhandene Installation oder gleichnamige Queue. Seine gesicherten Ergebnisse liegen anschließend unter `artifacts\smoke\`. Ohne `-TestPrint` prüft das Skript nur Paketinstallation und Queue-Lifecycle; genau diese Variante läuft in GitHub Actions, weil der verwendete gehostete Runner den Job in seiner nicht interaktiven Sitzung vor der Print-Workflow-Aktivierung abgebrochen hat.

## Testmatrix

| Test | Quelle | Inhalt | Erwartung |
|---|---|---|---|
| A | Notepad | eine Seite Text | PDF und Companion öffnen |
| B | Testsender + Word/Notepad | mehrere Seiten | automatisiert 3 Seiten bestätigt; Anwendungstest ausstehend |
| C | Testsender + Word | A4 Hochformat | Testsender bestätigt; Word ausstehend |
| D | Testsender + Word/Browser | A4 Querformat | Testsender bestätigt; Anwendungstest ausstehend |
| E | Testsender + Browser/Word | Text und Bild | PDF-Erzeugung automatisiert bestätigt; visuelle Anwendungskontrolle ausstehend |
| F | Testsender + Excel | Tabelle | PDF-Erzeugung automatisiert bestätigt; Excel ausstehend |
| G | zwei Testsender/Anwendungen | zwei schnelle Jobs | zwei getrennte UUID-PDFs automatisiert bestätigt |

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

Die systemnahen Informationen lassen sich ohne PDF-Inhalte automatisch sammeln:

```powershell
.\scripts\collect-diagnostics.ps1
```

Nur wenn die Job-Zuordnung untersucht werden muss, `-IncludeJobMetadata` ergänzen. JSON und JSONL können Dokumentnamen und Quellanwendungen enthalten; das Skript kopiert absichtlich niemals PDF-Dateien.
