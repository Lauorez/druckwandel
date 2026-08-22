# Windows Print API Notes

Stand: 23.08.2026

## Verifizierte Grundlage

- Die Virtual-Printer-APIs wurden mit Windows 11 24H2 / Build 26100 eingeführt. Deshalb sind Target Framework und Paket-MinVersion auf `10.0.26100.0` gesetzt.
- `windows.printSupportVirtualPrinterWorkflow` registriert die Queue gemeinsam mit dem MSIX-Paket; ein eigener V3-/V4-Treiber ist nicht erforderlich.
- `PreferredInputFormat="application/oxps"` liefert OXPS für normale Windows-Druckpfade.
- `PrintWorkflowPdlConverter` mit `PrintWorkflowPdlConversionType.XpsToPdf` übernimmt die lokale Konvertierung.
- Ein PDF-Passthrough wird ebenfalls akzeptiert, damit Anwendungen wie Browser ihren PDF-Stream ohne unnötige Zwischenkonvertierung übergeben können.
- `OutputFileTypes` ist absichtlich nicht gesetzt. Laut Microsoft würde dieses Attribut die Queue als File Printer markieren und den Windows-Speichern-unter-Dialog aktivieren.
- `PrintWorkflowConfiguration` liefert `SessionId`, `JobTitle` und `SourceAppDisplayName` über offizielle APIs.
- `PrintWorkflowUILauncher.LaunchAndCompleteUIAsync` aktiviert die Companion-UI über `windows.printSupportJobUI`.

## Primärquellen

- [MSIX Manifest Specification for Print Support Virtual Printer](https://learn.microsoft.com/windows-hardware/drivers/devapps/msix-manifest-specification-print-support-virtual-printer)
- [Print Support App v4 API Design Guide](https://learn.microsoft.com/windows-hardware/drivers/devapps/print-support-app-v4-design-guide)
- [PrintWorkflowVirtualPrinterTriggerDetails](https://learn.microsoft.com/uwp/api/windows.graphics.printing.workflow.printworkflowvirtualprintertriggerdetails?view=winrt-26100)
- [PrintWorkflowVirtualPrinterDataAvailableEventArgs](https://learn.microsoft.com/uwp/api/windows.graphics.printing.workflow.printworkflowvirtualprinterdataavailableeventargs?view=winrt-26100)
- [PrintWorkflowConfiguration](https://learn.microsoft.com/uwp/api/windows.graphics.printing.workflow.printworkflowconfiguration?view=winrt-26100)
- [Microsoft print-oem-samples – C# Windows App SDK](https://github.com/microsoft/print-oem-samples/tree/master/PSASamples/WinAppSdk/CSharp)

## Noch auf Windows zu verifizieren

- Der aktuelle C#-Samplepfad nennt selbst eine noch zu testende Aktivierungsprojektion für `PrintWorkflowJobActivatedEventArgs`.
- Verhalten von `LaunchAndCompleteUIAsync`, wenn mehrere Jobs nahezu gleichzeitig eintreffen.
- Validität der minimalen PDC-Datei bei der Paketinstallation.
- Ausgabequalität und PrintTicket-Übernahme bei Notepad, Browser, Word und Excel.
