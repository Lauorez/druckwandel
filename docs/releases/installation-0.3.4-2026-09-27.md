# Installation 0.3.4 – 27.09.2026

Setup 0.3.4 ergänzt die einmalige Administratorfreigabe für das Entwicklungszertifikat des Druckers. Die Anwendung und der Drucker werden anschließend weiterhin im normalen Benutzerkonto installiert. Das unveränderte Druckerpaket hat Version 0.1.0.12.

Wenn das Zertifikat im Computerspeicher `LocalMachine\TrustedPeople` fehlt, öffnet das Setup die Windows-Abfrage für Administratorrechte. Ein erhöhter Hilfsschritt prüft Paketidentität, Signatur und das öffentliche Zertifikat erneut und importiert nur dieses Zertifikat. Bei Ablehnung oder fehlenden Administratorrechten bricht das Setup ab. Auf einem verwalteten Arbeitsrechner kann die IT den Import auch vorab durchführen.

Der lokale Test entfernte das Zertifikat vorübergehend und führte zunächst das Installationsskript mit dem unveränderten Druckerpaket 0.1.0.12 aus dem Release 0.3.3 aus. Danach wurde auch die fertige Setup-Datei 0.3.4 unter derselben Bedingung ausgeführt. Die Administratorabfrage erschien, das Zertifikat wurde hinterlegt und das Setup endete mit Exitcode 0. Die installierte Anwendung und der Deinstallationseintrag melden 0.3.4; der Drucker `E-Rechnung` und das Paket 0.1.0.12 sind vorhanden. Der erhöhte Hilfsschritt verweigerte einen Aufruf ohne Administratorrechte.

Installer: `E-Rechnungs-Assistent-0.3.4-x64-Setup.exe` (464.671.218 Bytes). SHA-256: `723b4e0004899ad20e1eb66c6ebfb75bd16446e76b32ae2b82d75f50442cc661`. Der Windows-Build und die Installer-Tests liefen in [GitHub Actions](https://github.com/Lauorez/erechnung/actions/runs/36351685804) erfolgreich; auch das [portable Release-Gate](https://github.com/Lauorez/erechnung/actions/runs/36351685855) bestand. Der lokale Test prüfte die Zertifikatfreigabe und Installation, keinen erneuten echten Druckauftrag.

Das Setup ist weiterhin unsigniert und das Druckerpaket verwendet ein selbstsigniertes Entwicklungszertifikat. Eine Erstinstallation ohne Administratorfreigabe auf einem neuen Rechner erfordert eine öffentlich vertrauenswürdige Paketsignatur.
