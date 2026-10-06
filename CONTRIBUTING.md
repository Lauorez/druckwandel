# Mitwirken an Druckwandel

Danke, dass du zum Projekt beitragen möchtest! Fehlerberichte, Testergebnisse von echten Windows-Systemen, Dokumentationsverbesserungen und Code sind gleichermaßen willkommen. Issues und Pull Requests dürfen auf Deutsch oder Englisch verfasst werden.

## Bevor du loslegst

- Für größere Änderungen bitte zuerst ein Issue eröffnen und den Ansatz kurz abstimmen. Das spart beiden Seiten Arbeit.
- Sicherheitslücken bitte **nicht** öffentlich melden, sondern wie in [SECURITY.md](SECURITY.md) beschrieben.
- **Keine echten Rechnungen** in Issues, Pull Requests, Testfällen oder Screenshots. Rechnungen enthalten personenbezogene Daten, Bankverbindungen und Geschäftsgeheimnisse. Für reproduzierbare Fälle bitte den synthetischen Generator oder anonymisierte Beispiele verwenden, siehe [docs/corpus.md](docs/corpus.md).

## Entwicklungsumgebung

| Komponente | Voraussetzung |
| --- | --- |
| TypeScript-Kern und Oberfläche | Node.js 22 (siehe `.nvmrc`) |
| Tauri-Backend | Rust gemäß `rust-toolchain.toml`, plus die [Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/) deines Betriebssystems |
| Virtueller Drucker (`drucker/`) | Windows 11 24H2 (Build 26100+), .NET SDK 10, Visual Studio mit WinUI- und MSIX-Werkzeugen |

Fachkern, Tests und die Weboberfläche lassen sich auf Windows, macOS und Linux entwickeln. Nur der virtuelle Drucker und der Windows-Installer benötigen Windows.

```sh
npm install
npm run check            # TypeScript-Build, Unit-Tests und Korpus-Qualitätsgate
npm run desktop          # Tauri-App im Entwicklungsmodus
npm run desktop:web:dev  # nur die Oberfläche im Browser (ohne native Funktionen)
```

## Prüfgates vor einem Pull Request

Bitte vor dem Einreichen lokal ausführen, was deine Änderung betrifft:

```sh
npm run check          # Core- und Desktop-Typprüfung, Tests inkl. UI, Korpus und Fuzzing
npm run check:native   # Rust: Clippy mit -D warnings und cargo test
```

Änderungen an XRechnung-, ZUGFeRD- oder PDF/A-Ausgaben zusätzlich mit den gebündelten Prüfern verifizieren (die JRE dafür lädt `npm run validators:fetch`):

```sh
npm run check:xml      # KoSIT/Mustang-Regressionen
npm run check:pdf      # Hybrid-PDF mit veraPDF und Mustang
```

Für Änderungen am Drucker oder Installer zusätzlich auf Windows:

```powershell
.\drucker\scripts\test.ps1
npm run printer:build
npm run release:gate   # vollständiges Gate inkl. .NET, Prüfern und Installer-Szenarien
```

Die CI führt `npm run release:gate -- --portable` auf Ubuntu aus. Schritte, die Windows oder die gebündelten Prüfer brauchen, werden dort als „pending“ markiert und gelten nicht als bestanden.

## Leitlinien für Code

- **Geldbeträge sind Dezimal-Strings.** JavaScript-`number` ist für Beträge und Mengen im Domänenmodell tabu; gerechnet wird mit `decimal.js`.
- **Eine Rechnungslogik.** Fachliche Regeln, Berechnung und XML-Erzeugung leben ausschließlich im TypeScript-Kern unter `src/`. Rust (`apps/desktop/src-tauri`) ist die schmale Vertrauensgrenze für Dateien, SQLite und Prozessstarts; .NET (`drucker/`) nimmt nur Druckaufträge an.
- **Lokal bleibt lokal.** Produktcode sendet keine Rechnungsdaten an Netzwerkdienste und enthält keine Telemetrie.
- **Tests zu jedem Verhalten.** Neue Funktionen und Fehlerbehebungen brauchen passende Tests. Änderungen an der Extraktion müssen das Korpus-Gate (`npm run corpus:check`) bestehen.
- **Ehrliche Nachweise.** Ein Unit-Test ersetzt keinen Druck-, Installations- oder DATEV-Importtest. Ungeprüftes bleibt in der [Abnahmematrix](docs/acceptance-matrix.md) als ungeprüft stehen.
- Stil an den umgebenden Code anpassen. Die Grundeinstellungen stehen in `.editorconfig`.
- Die Oberfläche ist deutschsprachig und richtet sich an nicht-technische Anwender. Bitte verständliche Formulierungen statt Fachjargon wählen.

## Commits und Pull Requests

- Kleine, in sich geschlossene Pull Requests mit einer klaren Beschreibung, *warum* die Änderung nötig ist.
- Commit-Nachrichten im Imperativ, z. B. „Reject credit notes before export“.
- Nutzerrelevante Änderungen im Abschnitt „Unveröffentlicht“ des [CHANGELOG.md](CHANGELOG.md) ergänzen.
- Mit dem Einreichen eines Beitrags erklärst du dich einverstanden, dass er unter der [MIT-Lizenz](LICENSE) des Projekts veröffentlicht wird.

## Verhaltenskodex

Für alle Beteiligten gilt der [Verhaltenskodex](CODE_OF_CONDUCT.md).
