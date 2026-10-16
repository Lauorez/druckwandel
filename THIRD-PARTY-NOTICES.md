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

[Slint](https://slint.dev/) wird nach den Slint-Lizenzbedingungen verwendet. Der über das Zahnrad unter „Über Druckwandel“ erreichbare Bereich enthält die erforderliche Slint-Nennung. Rust-Bibliotheken einschließlich lopdf, pdf-extract und robius-share behalten ihre Paketlizenzen. Das kompakte RGB-ICC-Profil in `apps/native/assets/srgb.icc` trägt den eingebetteten CC0-Hinweis.

## PDF-Vorschau

[Hayro](https://github.com/LaurenzV/hayro) und [Vello CPU](https://github.com/linebender/vello) rendern die PDF-Vorschau lokal in Rust (jeweils Apache-2.0 oder MIT). Die mit Hayro eingebundenen Standard-Schriften und CMaps behalten ihre mitgelieferten Lizenzhinweise.

### Hayro (MIT)

```text
Copyright (c) The Hayro Authors

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

### Vello CPU (MIT)

```text
Copyright 2020 the Vello Authors

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### Eingebettete Standardschriften

```text
// Copyright 2014 PDFium Authors. All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//    * Redistributions of source code must retain the above copyright
// notice, this list of conditions and the following disclaimer.
//    * Redistributions in binary form must reproduce the above
// copyright notice, this list of conditions and the following disclaimer
// in the documentation and/or other materials provided with the
// distribution.
//    * Neither the name of Google Inc. nor the names of its
// contributors may be used to endorse or promote products derived from
// this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Adobe CMaps

```text
Copyright 1990-2023 Adobe. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

Redistributions of source code must retain the above copyright notice,
this list of conditions and the following disclaimer.

Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

Neither the name of Adobe nor the names of its contributors may be
used to endorse or promote products derived from this software without
specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

Das CGATS-Farbprofil von Hayro steht unter [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
