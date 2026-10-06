# Drittanbieter-Komponenten

Der Quellcode dieses Repositorys steht unter der [MIT-Lizenz](LICENSE). Die folgenden Komponenten Dritter unterliegen ihren eigenen Lizenzen.

## Gebündelte Prüfwerkzeuge (nicht im Repository)

`npm run validators:fetch` lädt diese Werkzeuge von den Herausgebern herunter und legt sie unter `apps/desktop/src-tauri/resources/validators/` ab. Der Windows-Installer liefert sie unverändert mit aus. Versionen und Quellen stehen in [`manifest.json`](apps/desktop/src-tauri/resources/validators/manifest.json), das vollständige Inventar aller gebündelten Bestandteile in [docs/components.md](docs/components.md).

| Komponente | Zweck | Lizenz |
| --- | --- | --- |
| [Eclipse Temurin JRE 21](https://adoptium.net/) | private Java-Laufzeit für die Prüfer | GPL-2.0 mit Classpath Exception |
| [KoSIT Validator](https://github.com/itplr-kosit/validator) und [XRechnung-Konfiguration](https://github.com/itplr-kosit/validator-configuration-xrechnung) | XRechnung-Prüfung | Apache-2.0 |
| [Mustang-CLI](https://www.mustangproject.org/) | ZUGFeRD/Factur-X-Prüfung und XML-Extrakt | Apache-2.0 |
| [veraPDF Greenfield](https://verapdf.org/) | PDF/A-3b-Prüfung | GPL-3.0-or-later oder MPL-2.0 |

Wer einen eigenen Installer mit diesen Werkzeugen verteilt, muss die jeweiligen Lizenzbedingungen einhalten.

## Assets

Die Platzhalter-Assets unter `drucker/src/CompanionApp/Assets` stammen aus [microsoft/print-oem-samples](https://github.com/microsoft/print-oem-samples) (MIT, Copyright (c) Microsoft Corporation). Der vollständige Lizenztext steht in [drucker/THIRD-PARTY-NOTICES.md](drucker/THIRD-PARTY-NOTICES.md).

## Bibliotheken

npm-, Cargo- und NuGet-Abhängigkeiten werden über die Paketmanager bezogen. Ihre Lizenzen stehen in den jeweiligen Paketen (`package-lock.json`, `apps/desktop/src-tauri/Cargo.lock`, `drucker/**/*.csproj`).
