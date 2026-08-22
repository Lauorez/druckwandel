# Projektübergabe – E-Rechnung Virtual PDF Printer

**Stand:** 23.08.2026  
**Version:** `0.1.0-beta.1`  
**Repository:** `https://github.com/Lauorez/erechnung` (privat)  
**Branch:** `main`

## Ziel und Scope

Dieses Repository implementiert ausschließlich den lokalen Windows-Print-Adapter des späteren E-Rechnung-Produkts. Ein Benutzer soll aus einer normalen Anwendung auf den Drucker `E-Rechnung` drucken, worauf Windows den Druckjob lokal in ein PDF konvertiert und eine kleine Companion-App aktiviert.

Nicht Bestandteil dieser Beta sind PDF-Analyse, OCR, Rechnungsfelder, EN 16931, ZUGFeRD, XRechnung, PDF/A, Datenbank, Cloud, Lizenzierung oder eine produktive Review-UI.

## Aktueller Implementierungsstand

Vorhanden sind:

- ein moderner Print Support Virtual Printer über `windows.printSupportVirtualPrinterWorkflow`,
- Windows 11 24H2 / Build 26100 als Mindestplattform,
- `application/oxps` als bevorzugtes Eingabeformat,
- OXPS-zu-PDF über `PrintWorkflowPdlConverter`,
- PDF-Passthrough bis PDF 1.7,
- bewusst kein `OutputFileTypes` im Manifest, damit die Queue kein klassischer File Printer mit Windows-Speichern-unter-Dialog wird,
- ein CsWinRT-Background-Task für Annahme, Konvertierung und Workflow-Abschluss,
- eine native WinUI-3-Companion-App für Job-ID, Dokumentname, Quellanwendung, PDF-Pfad, Seitenzahl und Status,
- Aktivierung der Companion-App über `PrintWorkflowUILauncher` und `windows.printSupportJobUI`,
- ein plattformunabhängiger `PrintJobStore` mit UUID-Dateinamen, atomaren JSON-Schreibvorgängen, SHA-256-Session-Index und JSONL-Statuslogs,
- getrennte Statuswerte für Empfang, Konvertierung, Companion-Start, Abschluss, Abbruch und Fehler,
- ein selbstenthaltenes x64-MSIX mit Development-Signatur,
- PowerShell-Skripte für Build, Zertifikat, Installation, Deinstallation, Tests, Smoke-Test und Diagnose,
- ein WPF-/XPS-basierter interaktiver Testsender,
- acht Unit-Tests für Store, JSON-Vertrag, Statusfehler, Parallelität und sichere Session-Schlüssel.

## Wichtige Dateien

| Bereich | Datei |
|---|---|
| Einstieg und Bedienung | `README.md` |
| Releasehistorie | `CHANGELOG.md` |
| Background-Workflow | `src/VirtualPrinter.Tasks/VirtualPrinterBackgroundTask.cs` |
| Companion-Aktivierung | `src/CompanionApp/App.xaml.cs` |
| Companion-Oberfläche | `src/CompanionApp/CompanionPage.xaml(.cs)` |
| Job-Store | `src/PrintCore/PrintJobStore.cs` |
| MSIX-/Druckerregistrierung | `src/CompanionApp/Package.appxmanifest` |
| Druckerfähigkeiten | `src/CompanionApp/Config/PrinterPdc.xml` |
| Paketbuild | `scripts/build.ps1` |
| Installation | `scripts/install.ps1` |
| Automatischer Desktop-Test | `scripts/smoke-install.ps1 -TestPrint` |
| Diagnose | `scripts/collect-diagnostics.ps1` |
| Manueller Testplan | `docs/testing.md` |
| API-Quellen und Entscheidungen | `docs/windows-print-api-notes.md` |

## Bereits praktisch bestätigt

Die GitHub-Actions-Windows-CI läuft auf einem frischen Windows-Build-26100-System. Folgendes wurde dort erfolgreich ausgeführt:

1. .NET-/WinUI-/CsWinRT-Release-Build,
2. Erzeugung eines Development-Zertifikats,
3. Erstellung und SHA-256-Signatur des MSIX,
4. Prüfung auf `AppxManifest.xml`, PDC, Background-DLL, WINMD, `WinRT.Host.dll`, PRI-Ressourcen und Signatur,
5. Prüfung, dass das fertige Manifest die Virtual-Printer-Erweiterung und kein `OutputFileTypes` enthält,
6. maschinenweites Vertrauen des Testzertifikats unter `LocalMachine\TrustedPeople`,
7. Installation des Pakets,
8. Erscheinen der Queue `E-Rechnung`,
9. Deinstallation des Pakets,
10. Verschwinden der Queue,
11. Build des interaktiven XPS-Testsenders,
12. datensparsame Diagnosesammlung,
13. acht grüne Unit-Tests unter Linux/macOS und CI.

Ein NuGet-Audit meldete keine bekannten verwundbaren direkten oder transitiven Pakete. CsWinRT ist auf der stabilen Version `2.3.1` und die .NET-10-Windows-SDK-Projektion auf `10.0.26100.87` festgesetzt.

## Was noch nicht bestätigt ist

Der tatsächliche interaktive Druckpfad wurde noch nicht auf einem angemeldeten Windows-11-Desktop ausgeführt. Insbesondere offen:

- Druck aus Notepad erzeugt ein PDF,
- kein Windows-Speichern-unter-Dialog erscheint,
- Companion-App startet über `PrintSupportJobUI`,
- Aktivierungsargumente lassen sich als `PrintWorkflowJobActivatedEventArgs` projizieren,
- Companion-App erhält exakt den richtigen Session-/Job-Datensatz,
- PDF lässt sich öffnen und entspricht dem Druckbild,
- mehrere Seiten, Querformat, Bilder, Tabellen und Farben,
- zwei schnelle Jobs bleiben getrennt,
- Word, Excel und Browser,
- ARM64.

Microsofts aktuelles C#-Sample nennt die Projektion der `PrintWorkflowJobActivatedEventArgs` selbst als praktisch noch zu bestätigenden Punkt. Die Beta enthält deshalb einen Fallback: Falls die Aktivierungsdaten nicht projiziert werden können, zeigt die Companion-App den neuesten lokalen Job und eine sichtbare Warnung.

## Erkenntnis aus dem Headless-CI-Druckversuch

Es wurden zwei automatisierte Drucksender im gehosteten Windows-Runner ausprobiert:

1. `System.Drawing.Printing.PrintDocument` scheiterte vor dem Produktworkflow mit Win32-Fehler 3003 (`A StartDocPrinter call was not issued`).
2. Ein moderner WPF-/XPS-Sender über `PrintQueue.AddJob` wurde in der nicht interaktiven Runner-Sitzung von Windows mit `PrintingCanceledException` abgebrochen.

In beiden Fällen entstand kein Paket-LocalState-Job; der Virtual-Printer-Background-Task wurde also nicht mit Druckdaten aktiviert. Deshalb prüft CI verbindlich Build, Paket, Installation und Queue-Lifecycle. Der echte Drucktest bleibt bewusst der interaktiven Desktop-Sitzung vorbehalten. Der XPS-Sender liegt weiterhin unter `tests/PrintSmokeSender` und wird in CI kompiliert.

## Nächster Test auf Windows 11

PowerShell als Administrator öffnen:

```powershell
git clone https://github.com/Lauorez/erechnung.git
cd erechnung
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\build.ps1
.\scripts\install.ps1
Get-Printer -Name "E-Rechnung"
```

Danach zuerst Notepad öffnen, eine kurze Textseite erstellen und auf `E-Rechnung` drucken. Prüfen:

1. kein zusätzlicher Speichern-unter-Dialog,
2. Companion-App erscheint,
3. Job-ID, Dokumentname und PDF-Pfad sind plausibel,
4. `Open PDF` funktioniert,
5. PDF-Inhalt entspricht dem Notepad-Dokument,
6. nach `Close` wird der Printjob abgeschlossen.

Alternativ führt folgender Befehl einen destruktiven Installations-/Testdruck-/Deinstallationslauf aus:

```powershell
.\scripts\smoke-install.ps1 -TestPrint
```

Der manuelle Notepad-Test ist für den ersten Lauf vorzuziehen, weil sich UI und Timing dabei besser beobachten lassen.

## Diagnose bei einem Fehler

Zuerst den Inhalt dieses Pfads prüfen:

```text
%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\ERechnung\
```

Relevante Unterordner:

```text
PrintJobs\<UUID>.pdf
PrintJobs\<UUID>.json
Logs\<UUID>.jsonl
Sessions\<SHA256>.txt
```

Anschließend Diagnosedaten sammeln:

```powershell
.\scripts\collect-diagnostics.ps1 -IncludeJobMetadata
```

Das Skript kopiert absichtlich keine PDFs. JSON-/JSONL-Dateien können jedoch Dokumentnamen und Quellanwendungen enthalten und müssen vor dem Teilen geprüft werden.

Für die nächste Arbeitssitzung sind besonders hilfreich:

- genaue Windows-Version aus `winver`,
- Quellanwendung,
- sichtbares Verhalten nach dem Klick auf `Drucken`,
- ob ein PDF/JSON/JSONL-Dateisatz entstand,
- Inhalt des letzten JSONL-Logs,
- Ausgabe des Diagnose-Skripts,
- Screenshot oder Wortlaut einer Fehlermeldung.

## Release- und Git-Status

Die erste Beta verwendet:

- SemVer: `0.1.0-beta.1`,
- MSIX-Version: `0.1.0.1`,
- Git-Tag: `v0.1.0-beta.1`,
- private GitHub-Prerelease mit x64-MSIX, öffentlichem Development-Zertifikat und SHA-256-Prüfsummen.

Der Beta-Commit muss vor dem Tagging gepusht sein und Core-Tests sowie Windows-Paket-CI müssen grün sein. Das private Development-Zertifikat (`.pfx`) darf nie veröffentlicht werden; nur das öffentliche `.cer` gehört zu den Release-Artefakten.

## Sinnvolle nächste Entwicklungsentscheidungen

Nach dem ersten Desktop-Test in dieser Reihenfolge weiterarbeiten:

1. Aktivierungs-/Konvertierungsfehler aus dem echten Lauf beheben.
2. Notepad-Einseitentest stabil grün bekommen.
3. Browser, Mehrseitigkeit und Querformat testen.
4. parallele Jobs praktisch testen.
5. erst danach UI-Komfort oder weitere PDC-Optionen ergänzen.
6. Für eine öffentliche Distribution ein echtes Code-Signing-Zertifikat und einen vertrauenswürdigen Updatekanal einführen.

Die Scope-Grenze bleibt bestehen: Noch keine E-Rechnungsanalyse oder Standardkonvertierung in dieses Repository einbauen, solange der Virtual-Printer-Pfad nicht praktisch bestätigt ist.
