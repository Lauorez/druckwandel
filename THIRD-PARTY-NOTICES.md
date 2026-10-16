# Drittanbieter-Komponenten

Der Quellcode dieses Repositorys steht unter der [MIT-Lizenz](LICENSE). Die folgenden Komponenten Dritter unterliegen ihren eigenen Lizenzen.

## Gebündelte Prüfwerkzeuge (nicht im Repository)

`python3 scripts/fetch-validators.py` lädt diese Werkzeuge von den Herausgebern herunter und legt sie unter `resources/validators/` ab. Der Windows-Installer liefert sie unverändert mit aus. Versionen und Quellen stehen in [`manifest.json`](resources/validators/manifest.json).

| Komponente | Zweck | Lizenz |
| --- | --- | --- |
| [Eclipse Temurin JRE 21](https://adoptium.net/) | private Java-Laufzeit für die Prüfer | GPL-2.0 mit Classpath Exception |
| [KoSIT Validator](https://github.com/itplr-kosit/validator) und [XRechnung-Konfiguration](https://github.com/itplr-kosit/validator-configuration-xrechnung) | XRechnung-Prüfung | Apache-2.0 |
| [Mustang-CLI](https://www.mustangproject.org/) | ZUGFeRD/Factur-X-Prüfung und XML-Extrakt | Apache-2.0 |
| [veraPDF Greenfield](https://verapdf.org/) | PDF/A-3b-Prüfung | GPL-3.0-or-later oder MPL-2.0 |

Wer einen eigenen Installer mit diesen Werkzeugen verteilt, muss die jeweiligen Lizenzbedingungen einhalten.

## Assets

Die Platzhalter-Assets unter `printer/src/CompanionApp/Assets` stammen aus [microsoft/print-oem-samples](https://github.com/microsoft/print-oem-samples) (MIT, Copyright (c) Microsoft Corporation). Der vollständige Lizenztext steht in [printer/THIRD-PARTY-NOTICES.md](printer/THIRD-PARTY-NOTICES.md).

## Bibliotheken

Cargo- und NuGet-Abhängigkeiten werden über die Paketmanager bezogen. Ihre Lizenzen stehen in den jeweiligen Paketen (`Cargo.lock`, `printer/**/*.csproj`).

## Desktop-Oberfläche

[Slint](https://slint.dev/) wird nach den Slint-Lizenzbedingungen verwendet. Der über „Über“ erreichbare Dialog enthält die erforderliche Slint-Nennung. Rust-Bibliotheken einschließlich lopdf, pdf-extract und robius-share behalten ihre Paketlizenzen. Das kompakte RGB-ICC-Profil in `apps/native/assets/srgb.icc` trägt den eingebetteten CC0-Hinweis.
