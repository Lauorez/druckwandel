# Architektur des Virtual-Printer-PoC

## Scope

Dieser Stand implementiert ausschließlich den lokalen Print Adapter und eine kleine Companion-App. PDF-Analyse, OCR, EN 16931, ZUGFeRD, XRechnung und Cloud-Komponenten sind nicht enthalten.

## Datenfluss

```text
Windows-Anwendung
  -> Drucker "E-Rechnung"
  -> Windows Print Pipeline (OXPS oder PDF-Passthrough)
  -> VirtualPrinterBackgroundTask
  -> PrintWorkflowPdlConverter.XpsToPdf
  -> Paket-LocalState/ERechnung/PrintJobs/<UUID>.pdf
  -> Paket-LocalState/ERechnung/PrintJobs/<UUID>.json
  -> PrintWorkflowUILauncher
  -> WinUI Companion-App
```

`OutputFileTypes` fehlt absichtlich im MSIX-Manifest. Dadurch ist die Queue kein klassischer File Printer und Windows zeigt keinen zusätzlichen Speichern-unter-Dialog.

## Komponenten

- `PrintCore`: plattformunabhängiges DTO, atomare Metadaten, Statuslog und Session-Index.
- `VirtualPrinter.Tasks`: WinRT-Background-Task. Er verarbeitet den PDL-Stream und schließt den Spooler-Job ab.
- `CompanionApp`: WinUI-3-Oberfläche, aktiviert über `windows.printSupportJobUI`.

## Parallelität und Übergabe

Jeder Job erhält eine zufällige UUID. Die Windows-`SessionId` wird ausschließlich als SHA-256-Schlüssel in einem Session-Index verwendet; unkontrollierte Jobtitel werden nie zu Dateinamen. Die UI erhält dieselbe Session-ID über die offizielle Print-Workflow-Konfiguration und kann damit genau den passenden Job laden.

Metadaten werden über temporäre Dateien und einen abschließenden Rename geschrieben. PDFs werden zuerst als `<UUID>.pdf.tmp` erzeugt und nach erfolgreicher Konvertierung umbenannt.

## Lokaler Speicher

Der PoC verwendet bewusst `ApplicationData.Current.LocalFolder`. Der physische Pfad entspricht typischerweise:

```text
%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\ERechnung\
```

Damit benötigen Background-Task und Companion keine zusätzliche Dateisystemberechtigung. Eine spätere Produkt-App kann die Speicherimplementierung hinter `PrintJobStore` austauschen.
