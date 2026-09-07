# Architektur der Windows-Druckbrücke

## Scope

Dieses Teilprojekt implementiert ausschließlich die lokale Windows-Druckannahme. PDF-Analyse, Prüfung, Ausgabe und Archivierung liegen in der Tauri-Anwendung des Hauptprojekts.

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
  -> WinUI-Druckbrücke (windows.printSupportJobUI)
  -> Dokumente/E-Rechnung Druckeingang/<UUID>.pdf + <UUID>.printjob.json
  -> erechnung-review://print-job/<UUID>
  -> Tauri E-Rechnungs-Assistent
  -> <UUID>.review.json
```

`OutputFileTypes` fehlt absichtlich im MSIX-Manifest. Dadurch ist die Queue kein klassischer File Printer und Windows zeigt keinen zusätzlichen Speichern-unter-Dialog.

## Komponenten

- `PrintCore`: plattformunabhängiges DTO, atomare Metadaten, Statuslog, Session-Index und versionierter Review-Handoff.
- `VirtualPrinter.Tasks`: WinRT-Background-Task. Er verarbeitet den PDL-Stream und schließt den Spooler-Job ab.
- `CompanionApp`: technisch notwendiger WinUI-3-Endpunkt für `windows.printSupportJobUI`; automatische Übergabe an Tauri, sichtbare Oberfläche nur als Fehlerfallback.

Die WinUI-Komponente ist absichtlich keine zweite Fachanwendung. Microsofts Print-Support-Vertrag aktiviert den Vordergrundteil über `PrintWorkflowUILauncher`; der paketisolierte Background-Task bleibt für Konvertierung und Abschluss des Spooler-Jobs zuständig. Rechnungsanalyse und Benutzerbearbeitung gehören ausschließlich in die Tauri-App.

## Parallelität und Übergabe

Jeder Job erhält eine zufällige UUID. Die Windows-`SessionId` wird ausschließlich als SHA-256-Schlüssel in einem Session-Index verwendet; unkontrollierte Jobtitel werden nie zu Dateinamen. Die UI erhält dieselbe Session-ID über die offizielle Print-Workflow-Konfiguration und kann damit genau den passenden Job laden.

Metadaten werden über temporäre Dateien und einen abschließenden Rename geschrieben. PDFs werden zuerst als `<UUID>.pdf.tmp` erzeugt und nach erfolgreicher Konvertierung umbenannt.

Die Druckbrücke kopiert ausschließlich ein validiertes PDF mit `%PDF-`-Header in den Übergabeordner. Der Deep Link enthält nur die UUID, niemals einen frei wählbaren Dateipfad. Der E-Rechnungs-Assistent löst diese UUID gegen die lokalen Metadaten auf, beschränkt Dateizugriffe auf den kanonischen Eingangspfad und bestätigt Öffnen oder Fehler in `<UUID>.review.json`.

## Lokaler Speicher

Der PoC verwendet bewusst `ApplicationData.Current.LocalFolder`. Der physische Pfad entspricht typischerweise:

```text
%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalState\ERechnung\
```

Damit benötigen Background-Task und Companion keine zusätzliche Dateisystemberechtigung. Eine spätere Produkt-App kann die Speicherimplementierung hinter `PrintJobStore` austauschen.
