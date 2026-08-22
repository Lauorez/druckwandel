# Auftrag: Windows Virtual PDF Printer PoC

Du arbeitest an einem größeren Projekt namens **E-Rechnung-MVP**.

Für diesen Auftrag soll **ausschließlich der virtuelle Windows-PDF-Drucker** entwickelt werden.

Es geht ausdrücklich **noch nicht** um E-Rechnungen.

## Kontext des späteren Produkts

Langfristig soll der Benutzer beispielsweise in:

- Microsoft Word
- Microsoft Excel
- Browsern
- älteren ERP-/Faktura-Anwendungen
- Handwerkersoftware
- beliebigen normalen Windows-Anwendungen

auf:

> Drucken → „E-Rechnung“

klicken können.

Der virtuelle Drucker soll den Druckjob lokal übernehmen, daraus ein PDF erzeugen und anschließend die eigentliche E-Rechnungs-Anwendung öffnen.

Diese spätere Anwendung wird PDF-Extraktion, Review, ZUGFeRD, XRechnung usw. übernehmen.

**Nichts davon ist Bestandteil dieses Auftrags.**

---

# Ziel dieses Auftrags

Baue einen funktionsfähigen Proof of Concept eines modernen virtuellen PDF-Druckers unter Windows.

Nach Installation soll in Windows ein Drucker erscheinen, beispielsweise:

> E-Rechnung

Wenn ein Benutzer aus einer beliebigen normalen Windows-Anwendung auf diesen Drucker druckt, soll:

1. der Windows-Printjob vom virtuellen Drucker entgegengenommen werden,
2. der Druckinhalt in ein normales PDF konvertiert werden,
3. das PDF automatisch lokal in einem kontrollierten Arbeitsverzeichnis gespeichert werden,
4. anschließend eine lokale Companion-App bzw. ein Testprogramm gestartet oder aktiviert werden,
5. dieser Anwendung der Pfad zum erzeugten PDF sowie Metadaten des Druckjobs übergeben werden.

Der gesamte Vorgang muss lokal funktionieren.

Es darf keinerlei Cloud-Komponente geben.

---

# WICHTIGE SCOPE-GRENZE

Implementiere **ausschließlich den virtuellen Drucker und dessen Übergabe an eine Companion-App**.

Nicht implementieren:

- PDF-Inhalt analysieren
- OCR
- Rechnungsfelder erkennen
- Rechnungsnummer erkennen
- Rechnungssummen erkennen
- EN 16931
- ZUGFeRD
- Factur-X
- XRechnung
- XML
- PDF/A-3
- Rechnung validieren
- Datenbank
- Benutzerkonto
- Lizenzierung
- Cloud
- API-Backend
- KI
- Electron-Anwendung
- finale Produkt-UI

Wenn etwas davon für einen Test erforderlich erscheint, stattdessen einen kleinen Mock/Stubs verwenden.

---

# Zielplattform

Primär:

- Windows 11 x64

Die Architektur soll nach Möglichkeit auch für Windows on ARM sinnvoll bleiben, sofern die verwendeten Windows-APIs dies erlauben.

Keine Legacy-Unterstützung erzwingen, wenn sie die Architektur verschlechtert.

---

# Architekturentscheidung

Untersuche und verwende bevorzugt Microsofts aktuelle Architektur für Softwaredrucker:

- Print Support App
- Print Support Virtual Printer
- `windows.printSupportVirtualPrinterWorkflow`
- `Windows.Graphics.Printing.PrintSupport`
- MSIX/Appx-basierte Registrierung

Verwende **keinen eigenen klassischen V3/V4-Kernel-/Printer-Driver**, sofern die moderne Microsoft-Architektur den gewünschten Workflow ermöglicht.

Keine RedMon-/Ghostscript-Bastellösung als primäre Architektur.

Microsoft stellt für Virtual Printer Workflows unter anderem folgende Mechanismen bereit:

- PrintWorkflowVirtualPrinterSession
- VirtualPrinterDataAvailable
- PrintWorkflowVirtualPrinterDataAvailableEventArgs
- PrintWorkflowPdlSourceContent
- PrintWorkflowPdlConverter
- XPS/OXPS → PDF-Konvertierung
- PrintWorkflowUILauncher

Verifiziere die aktuell dokumentierten APIs anhand der Microsoft-Dokumentation, bevor du die Architektur implementierst.

Wenn API-Versionen oder Plattformanforderungen relevant sind, dokumentiere sie.

---

# Gewünschter Print-Workflow

Ziel:

```text
Word / Excel / Browser / Legacy Software
                │
                ▼
            Drucken
                │
                ▼
        Drucker „E-Rechnung“
                │
                ▼
      Windows Print Pipeline
                │
                ▼
   Virtual Printer Background Task
                │
                ▼
           PDL / OXPS
                │
                ▼
         XPS/OXPS → PDF
                │
                ▼
   %LOCALAPPDATA%\ERechnung\PrintJobs\
                │
                ▼
          job-UUID.pdf
                │
                ▼
        Companion Launcher
                │
                ▼
      Test-App zeigt Job an
```

---

# Kein Windows-„Speichern unter“

Der Benutzer soll nach Auswahl des Druckers **nicht zusätzlich einen Windows-Dateidialog zum Speichern des PDFs erhalten**.

Das PDF ist ein internes Zwischenartefakt.

Deshalb soll die Virtual-Printer-Konfiguration nach Möglichkeit nicht als klassischer `Print to File`-Drucker arbeiten.

Insbesondere soll geprüft werden, ob `OutputFileTypes` im Manifest weggelassen werden sollte, damit Windows keinen eigenen „Speichern unter“-Dialog anzeigt.

Die Anwendung entscheidet selbst über den temporären Dateipfad.

---

# Speicherung der erzeugten PDFs

Verwende beispielsweise:

```text
%LOCALAPPDATA%\ERechnung\PrintJobs\
```

Pro Printjob:

```text
<UUID>.pdf
<UUID>.json
```

Beispiel:

```text
8f98b985-294f-46ae-991b-04b9c81d387c.pdf
8f98b985-294f-46ae-991b-04b9c81d387c.json
```

Das JSON enthält mindestens:

```json
{
  "jobId": "...",
  "createdAt": "...",
  "pdfPath": "...",
  "printerName": "...",
  "documentName": "...",
  "sourceApplication": null,
  "status": "created"
}
```

`sourceApplication` nur erfassen, wenn dies über saubere Windows-APIs zuverlässig verfügbar ist.

Keine heuristischen Hacks dafür einbauen.

---

# PDF-Anforderungen

Für diesen PoC genügt ein normales PDF.

Noch nicht erforderlich:

- PDF/A
- PDF/A-3
- XML Attachment
- ZUGFeRD-Metadaten
- elektronische Signatur

Das PDF sollte visuell möglichst exakt dem entsprechen, was der normale Windows-Druckpfad erzeugt.

Wichtig sind insbesondere:

- DIN A4
- mehrere Seiten
- Hochformat
- Querformat
- Text
- Bilder
- Tabellen
- unterschiedliche Fonts
- Farben
- PrintTicket-Einstellungen

---

# Companion-App

Erstelle zusätzlich eine **sehr kleine Test-Companion-App**.

Keine produktive UI.

Technologie bevorzugt:

- C#/.NET

Die Anwendung soll lediglich demonstrieren, dass die Übergabe funktioniert.

Wenn ein Printjob abgeschlossen wurde, soll sie beispielsweise anzeigen:

```text
Print job received

Document:
Testrechnung 1234

PDF:
C:\Users\...\AppData\Local\ERechnung\PrintJobs\<UUID>.pdf

Pages:
3

Status:
Ready
```

Buttons:

- `Open PDF`
- `Open Folder`
- `Close`

Mehr UI ist nicht erforderlich.

---

# Übergabe vom Print-Workflow an die Companion-App

Entwirf die Kommunikation so, dass sie später problemlos gegen eine echte Desktop-Anwendung ausgetauscht werden kann.

Mögliche Varianten untersuchen:

- URI Protocol Activation
- App Service
- Named Pipe
- lokale IPC
- Activation Arguments
- andere offizielle Windows-Mechanismen

Bevorzuge eine robuste und einfache Windows-native Lösung.

Die Print-Komponente darf nicht eng an die Test-UI gekoppelt sein.

Definiere eine klare Boundary wie:

```text
Print Adapter
    ↓
PrintJob DTO
    ↓
Companion Launcher / IPC
```

---

# Race Conditions / mehrere Jobs

Berücksichtige bereits im PoC:

- zwei nahezu gleichzeitig eintreffende Printjobs
- Companion-App läuft bereits
- Companion-App läuft noch nicht
- gleicher Dokumentname mehrfach
- Druckjob wird abgebrochen
- PDF-Konvertierung schlägt fehl

Deshalb:

- UUID pro Job
- keine Dateinamen aus Dokumentnamen ableiten
- atomare bzw. sichere Dateierzeugung
- saubere Statusbehandlung

---

# Fehlerbehandlung

Fehler dürfen nicht still verschluckt werden.

Mindestens folgende Fehlerfälle unterscheiden:

```text
PrintJobReceived
PdfConversionStarted
PdfConversionSucceeded
PdfConversionFailed
CompanionLaunchSucceeded
CompanionLaunchFailed
JobCompleted
JobFailed
JobCanceled
```

Lokales Logging implementieren, beispielsweise unter:

```text
%LOCALAPPDATA%\ERechnung\Logs\
```

Keine Telemetrie.

Keine Netzwerkkommunikation.

---

# Datenschutz / Security

Das spätere Produkt ist Local-first.

Dieser PoC muss deshalb ebenfalls vollständig lokal arbeiten.

Nicht zulässig:

- Upload
- Cloud Logging
- externe API
- Analytics
- Crash-Upload
- Telemetrie
- externe PDF-Konverter

Alle Rechnungs-/Druckdaten bleiben lokal auf dem Rechner.

Temporäre Dateien dürfen nicht in unsicheren globalen Temp-Pfaden mit vorhersagbaren Namen abgelegt werden.

---

# Packaging

Ein wichtiger Teil dieses PoC ist nicht nur der Code, sondern die tatsächliche Installation.

Erstelle soweit möglich:

- MSIX/Appx-Konfiguration
- Manifest
- notwendige Capabilities
- Virtual-Printer-Registrierung
- PDC/PDR-Dateien, falls erforderlich
- Build-Skripte
- Installationsanleitung
- Deinstallationsanleitung

Nach Installation soll der Drucker automatisch in Windows erscheinen.

Nach Deinstallation soll er sauber verschwinden.

---

# Development Certificate

Für den PoC darf ein lokales Development-/Self-Signed-Zertifikat verwendet werden.

Erstelle eine dokumentierte Development-Prozedur.

Beispielsweise:

```text
scripts/
  create-dev-cert.ps1
  install-dev-cert.ps1
  build.ps1
  install.ps1
  uninstall.ps1
```

Produktives Code Signing ist noch nicht Bestandteil dieses Auftrags.

---

# Repository-Struktur

Halte das Repository übersichtlich.

Eine mögliche Struktur wäre:

```text
/
├─ README.md
├─ docs/
│  ├─ architecture.md
│  ├─ windows-print-api-notes.md
│  └─ testing.md
│
├─ src/
│  ├─ VirtualPrinter/
│  ├─ PrintCore/
│  └─ CompanionApp/
│
├─ packaging/
│  ├─ Package.appxmanifest
│  ├─ Config/
│  └─ Assets/
│
├─ scripts/
│  ├─ build.ps1
│  ├─ install.ps1
│  ├─ uninstall.ps1
│  └─ create-dev-cert.ps1
│
└─ tests/
```

Passe die Struktur an die tatsächlichen technischen Anforderungen der Windows Print Support App an.

Keine künstliche Architektur erzwingen, wenn die Microsoft-Projektstruktur etwas anderes erfordert.

---

# Technologiepräferenz

Bevorzugt:

```text
C#
.NET
WinUI / Windows App SDK, falls sinnvoll
Windows SDK / WinRT
MSIX
PowerShell für Build-/Install-Skripte
```

Native C++-Komponenten nur dann verwenden, wenn sie technisch tatsächlich notwendig sind.

Begründe solche Entscheidungen im Architektur-Dokument.

---

# Kein unnötiger Frontend-Stack

Für diesen PoC ausdrücklich nicht verwenden:

- Electron
- React
- Node.js
- WebView-basierte UI

Die Aufgabe besteht hauptsächlich aus Windows-Systemintegration.

Ein kleiner nativer C#-Client reicht.

---

# Testfälle

Der PoC muss mindestens mit folgenden Programmen getestet bzw. für Tests vorbereitet werden:

1. Windows Notepad
2. Microsoft Edge oder Chrome
3. Microsoft Word, falls installiert
4. Microsoft Excel, falls installiert
5. Windows Test Page / systemeigener Drucktest

Testdokumente:

### Test A

Eine Seite Text.

### Test B

Mehrseitiges Dokument.

### Test C

DIN A4 Hochformat.

### Test D

DIN A4 Querformat.

### Test E

Text + Bild.

### Test F

Tabelle.

### Test G

Zwei direkt nacheinander gestartete Printjobs.

---

# Acceptance Criteria

Der PoC gilt als erfolgreich, wenn folgende Sequenz auf einem frischen unterstützten Windows-11-System reproduzierbar funktioniert:

```text
1. Projekt bauen
2. Paket installieren
3. Windows zeigt Drucker „E-Rechnung“
4. Notepad öffnen
5. Text eingeben
6. Drucken
7. „E-Rechnung“ auswählen
8. Drucken
9. KEIN zusätzlicher Speichern-unter-Dialog
10. PDF wird automatisch lokal erzeugt
11. Companion-App öffnet sich
12. Companion-App kennt Job-ID und PDF-Pfad
13. PDF lässt sich öffnen
14. PDF entspricht dem gedruckten Dokument
15. Anwendung deinstallieren
16. Drucker verschwindet wieder
```

---

# Zweite Acceptance-Ebene

Zusätzlich prüfen:

```text
Word → E-Rechnung → PDF
Browser → E-Rechnung → PDF
mehrseitig → PDF
Querformat → PDF
zwei Jobs gleichzeitig → zwei getrennte PDFs
```

---

# Dokumentation

Erstelle eine ausführliche `README.md`.

Sie muss enthalten:

## Voraussetzungen

- Windows-Version
- Visual-Studio-Version
- notwendige Workloads
- .NET-Version
- Windows SDK
- eventuell erforderlicher Developer Mode

## Build

Exakte Befehle.

## Installation

Exakte Befehle.

## Test

Exakte Schritte.

## Uninstall

Exakte Schritte.

## Architektur

Kurze Erklärung des Druckpfads.

## Einschränkungen

Alles, was aktuell noch nicht zuverlässig funktioniert.

---

# Rechercheanforderung

Bevor du implementierst, recherchiere die aktuelle offizielle Microsoft-Dokumentation zu:

- Print Support App
- Print Support App v4 API
- Print Support Virtual Printer
- `windows.printSupportVirtualPrinterWorkflow`
- `PrintWorkflowVirtualPrinterSession`
- `PrintWorkflowPdlConverter`
- XPS/OXPS to PDF
- MSIX manifest requirements

Verwende möglichst Microsoft Learn bzw. offizielle Microsoft-Samples als Primärquelle.

Keine Architektur ausschließlich auf alten Blogposts oder StackOverflow-Antworten aufbauen.

Dokumentiere relevante Quellen in:

```text
docs/windows-print-api-notes.md
```

---

# Wichtige technische Entscheidung

Prüfe explizit folgenden gewünschten Ansatz:

```text
PreferredInputFormat = application/oxps
↓
Windows liefert OXPS
↓
PrintWorkflowPdlConverter
↓
PrintWorkflowPdlConversionType.XpsToPdf
↓
PDF Stream
↓
lokale Datei
```

Wenn dieser Ansatz mit der aktuellen API funktioniert, verwende ihn.

Wenn nicht, dokumentiere präzise:

1. warum nicht,
2. welche Plattform/API-Beschränkung existiert,
3. welche moderne Alternative Microsoft vorsieht.

Nicht stillschweigend auf einen Legacy-Druckertreiber wechseln.

---

# Vorgehensweise

Arbeite autonom.

Nicht nach jeder Kleinigkeit nachfragen.

Gehe iterativ vor:

1. Microsoft-API verifizieren
2. minimalen Virtual Printer erzeugen
3. Installation testen
4. PDL empfangen
5. PDF-Konvertierung implementieren
6. lokale Speicherung implementieren
7. Companion Activation implementieren
8. Fehlerbehandlung implementieren
9. Tests durchführen
10. README vervollständigen

Wenn Teile auf der vorhandenen Entwicklungsumgebung nicht ausführbar sind, implementiere sie trotzdem soweit belastbar möglich und dokumentiere exakt, was auf echter Windows-Hardware noch verifiziert werden muss.

---

# Prioritäten

In dieser Reihenfolge:

1. **Tatsächlich installierbarer virtueller Drucker**
2. **Printjob zuverlässig erhalten**
3. **gültiges PDF erzeugen**
4. **kein Save-As-Dialog**
5. **Companion-App starten**
6. **saubere Architektur**
7. **Fehlerbehandlung**
8. **Packaging**
9. **Tests**
10. kosmetische UI

Nicht mehrere Stunden in UI-Polishing investieren, solange der Druckpfad nicht zuverlässig funktioniert.

---

# Definition of Done

Am Ende möchte ich kein Architekturkonzept und keinen Dummy-Code, sondern einen möglichst weitgehend funktionierenden Repository-Stand.

Das zentrale Erfolgskriterium lautet:

> Ich installiere das Paket, sehe unter Windows den Drucker „E-Rechnung“, drucke aus einer normalen Anwendung darauf und wenige Augenblicke später liegt das erzeugte PDF lokal vor und wird meiner Companion-App übergeben.

Alles andere ist für diesen Auftrag sekundär.