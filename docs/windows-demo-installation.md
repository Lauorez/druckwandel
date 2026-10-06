# Vollständiges Windows-Vorführpaket

## Aktueller Stand vom 27.09.2026

Anwendung **0.3.4** ergänzt die Administratorabfrage für das Entwicklungszertifikat. Der Drucker **0.1.0.12** bleibt unverändert. Der [Installationsbericht 0.3.4](releases/installation-0.3.4-2026-09-27.md) beschreibt den neuen Ablauf. Die reale Druckprüfung aus [0.3.3](releases/installation-0.3.3-2026-09-27.md) bleibt die Referenz für den NativeAOT-Drucker.

## Prüfergebnis vom 11.09.2026

**Noch keine Freigabe für eine vollständige Druckvorführung.** Das gemeinsame Setup wurde auf Windows 11 25H2 (Build 26200) im normalen Benutzerkonto ausgeführt und endete mit Exitcode 0. Anwendung 0.3.1 und Drucker 0.1.0.5 sind installiert. Die Anwendung ist geöffnet und die Musterrechnung als Entwurf übernommen. Die installierte Java-Laufzeit und alle Validatoren sind vorhanden; die Druckoberfläche startet ebenfalls.

10 .NET-Core-Tests, 4 Installer-Konfigurationstests sowie die native COM-Aktivierung einschließlich `IBackgroundTask` ohne CoreCLR bestehen. **Der reale Drucktest besteht nicht:** WPF/XPS meldet `PrintingCanceledException`, GDI und direkte Spooler-Ansteuerung scheitern bei `StartDocPrinter`, bevor neue Druckjob-Artefakte entstehen. Microsoft Print to PDF funktioniert als Kontrolltest. Ein Neustart des eigenen PrintWorkflow-Benutzerdienstes und erneute Paketregistrierung behoben den Fehler nicht. Die alte Version 0.1.0.4 wurde kurz zum Vergleich installiert, registrierte dabei keine Warteschlange innerhalb von 40 Sekunden; anschließend wurde 0.1.0.5 wiederhergestellt. Aus diesem Vergleich lässt sich keine gesicherte Fehlerursache ableiten.

Der systemweite Spooler, Windows-Sicherheitsrichtlinien und Zertifikatsspeicher wurden nicht verändert. Ein Windows-Neustart mit anschließendem Wiederholungstest ist noch offen; es wurde kein automatischer Neustart ausgelöst. Unabhängig davon bleibt die fehlende öffentliche Paketsignatur die Grenze für eine erstmalige vollständige Installation auf einem fremden Rechner ohne Admin.

Voraussetzung: Windows 11 24H2 oder neuer, x64. Die Setup-Datei installiert Anwendung und Drucker für den aktuellen Benutzer und enthält:

- E-Rechnungs-Assistent 0.3.1 mit lokalem Archiv und Posteingang
- E-Rechnungsdrucker 0.1.0.5 inklusive nativ kompiliertem Background-Task
- .NET-Laufzeit der Druckoberfläche und Microsoft Windows App Runtime 1.8
- WebView2-Offline-Installer
- Java 21, KoSIT, Mustang, veraPDF und XRechnung-Regelpaket

Node, Rust, .NET SDK und eine gesonderte Java-Installation sind auf dem Zielrechner nicht nötig. Die Installation lädt diese Komponenten nicht aus dem Internet nach. Die Windows-Registrierung eines virtuellen Druckers benötigt die genannte Windows-Version.

## Wichtige Grenze des aktuellen Vorführbuilds

Es ist kein öffentlich vertrauenswürdiges Code-Signing-Zertifikat vorhanden. Das Drucker-MSIX ist deshalb mit dem Entwicklungszertifikat signiert. Ab Setup 0.3.4 prüft der Installer bei der ersten Installation, ob Windows diesem Zertifikat vertraut. Falls nicht, erscheint eine Windows-Abfrage für Administratorrechte. Der erhöhte Hilfsschritt prüft Paketidentität, Signatur und Zertifikat erneut und hinterlegt nur das passende öffentliche Zertifikat in `LocalMachine\TrustedPeople`. Danach installiert das normale Benutzerkonto Anwendung und Drucker.

Auf einem anderen Rechner gibt es zwei reguläre Wege:

1. Für die spätere Verteilung: Druckerpaket und Setup öffentlich vertrauenswürdig signieren. Danach kann ein Standardbenutzer installieren, soweit lokale Unternehmensrichtlinien die App erlauben.
2. Für einen beaufsichtigten Vorführtest: Eine Person mit Administratorrechten bestätigt die Abfrage des Setups. Alternativ kann die IT das öffentliche Entwicklungszertifikat vorab im lokalen Computerspeicher `TrustedPeople` hinterlegen. Dieser einmalige Vertrauensschritt braucht Administratorrechte.

Das Setup aktiviert keinen Entwicklermodus und startet keine Systemdienste neu. Wird die Administratorabfrage abgelehnt oder durch eine Unternehmensrichtlinie verhindert, bricht die vollständige Installation mit einer Fehlermeldung ab. Es gibt keinen als vollständig ausgegebenen App-only-Fallback.

Die Anforderung „auf einem beliebigen fremden Rechner erstmalig vollständig ohne Admin“ bleibt ohne vertrauenswürdige Signatur unerfüllt. Windows-Zertifikatvertrauen lässt sich nicht durch eine anders verpackte EXE ersetzen. Siehe [Microsoft: MSIX-Signierung](https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide) und [MSIX-Zertifikatfehler](https://learn.microsoft.com/en-us/windows/msix/msix-troubleshooting-guide).

## Bauen und installieren

```powershell
npm run validators:fetch
npm run installer:windows
```

Das Build-Skript prüft die Vollständigkeit der Validatoren und des Druckerpakets. Der native Drucker-Task wird vor dem MSIX-Paket mit .NET NativeAOT gebaut. Die JSON-Verarbeitung verwendet generierte Typinformationen; der Hintergrundprozess benötigt kein global installiertes .NET und keinen `WinRT.Host.dll`-Bootstrapper. Die Hauptanwendung bleibt ein Tauri-Release-Build.

Ergebnis: `artifacts/windows/Druckwandel-<Version>-x64-Setup.exe` (bis 0.3.4 `E-Rechnungs-Assistent-<Version>-x64-Setup.exe`). Die öffentliche `.cer` liegt nur für den beschriebenen Administratorschritt im Paket. Die private `.pfx` darf nicht in das Vorführpaket. Eine öffentlich vertrauenswürdige Signatur ist damit nicht vorhanden.

Das Setup normal starten, nicht „Als Administrator“. Bei einem Update legt das Setup zuerst einen Snapshot unter `%LOCALAPPDATA%\de.erechnung.converter\update-backup\<Version>` an (`WIEDERHERSTELLUNG.txt` im selben Ordner). Arbeitsentwürfe und `Dokumente\E-Rechnungsarchiv` bleiben unangetastet. Eine kleinere Versionsnummer als die bereits installierte wird abgewiesen. Das Setup aktualisiert den Drucker nur, wenn die Paketversion neuer ist. Die Deinstallation entfernt den Drucker, nicht das Archiv.

`printer/scripts/test-native-task.ps1` aktiviert den nativen Background-Task in einem Windows-PowerShell-Prozess und prüft, dass kein CoreCLR geladen wird. Der Test ist Teil von `npm run printer:build`. `printer/scripts/test.ps1` prüft weiterhin die persistierten Job- und Übergabeformate.

## Vorführung

1. Druckwandel über das Startmenü öffnen.
2. Die mitgelieferte `muster-rechnung.pdf` über „PDF öffnen“ laden und Angaben prüfen.
3. Die Rechnung zusätzlich aus einem PDF-Programm auf **E-Rechnung** drucken. Der Beleg soll automatisch im Rechnungseingang erscheinen; kein Speichern-unter-Dialog.
4. Angaben und Übereinstimmung bestätigen, XRechnung bzw. PDF-Rechnung erzeugen. Die unabhängigen Prüfer laufen lokal.
5. Den entstandenen Eintrag im Archiv zeigen.

Ein DATEV-Export braucht ein fachlich passendes Profil. Die Vorführung ersetzt keinen echten DATEV-Testimport.

Installationsdiagnose: `%TEMP%\Druckwandel-Installation.log` (bis 0.3.4 `%TEMP%\E-Rechnungs-Assistent-Installation.log`). Druckübergaben: `Dokumente\E-Rechnung Druckeingang` mit PDF, `.printjob.json` und Bestätigung `.review.json`.
