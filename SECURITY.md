# Sicherheitsrichtlinie

## Unterstützte Versionen

Das Projekt befindet sich in der Beta-Phase. Sicherheitskorrekturen erfolgen ausschließlich für den aktuellen Stand des Branches `main` und die jeweils neueste Vorabversion.

## Eine Sicherheitslücke melden

Bitte melde Schwachstellen **nicht** über öffentliche Issues, Diskussionen oder Pull Requests.

Nutze stattdessen die private Meldefunktion von GitHub: **Security → Report a vulnerability** im Repository ([direkter Link](https://github.com/Lauorez/druckwandel/security/advisories/new)).

Hilfreich sind:

- betroffene Komponente (TypeScript-Kern, Tauri/Rust-Backend, virtueller Drucker, Installer),
- betroffene Version bzw. Commit,
- Schritte zur Reproduktion oder ein Proof of Concept,
- die eingeschätzte Auswirkung.

Bitte keine echten Rechnungen oder personenbezogenen Daten beifügen. Du erhältst in der Regel innerhalb von sieben Tagen eine erste Rückmeldung. Nach einer Korrektur veröffentlichen wir auf Wunsch einen Hinweis mit Nennung der meldenden Person.

## Sicherheitsmodell in Kürze

- Rechnungsdaten werden ausschließlich lokal verarbeitet; der Produktcode baut keine Netzwerkverbindungen auf und enthält keine Telemetrie.
- Der Rust-Teil der Tauri-Anwendung ist die Vertrauensgrenze für Dateipfade, SQLite, Prozessstarts (gebündelte Java-Prüfer) und Deep Links (`erechnung-review://`).
- Die Windows-Druckbrücke übergibt Aufträge ausschließlich über UUID-basierte Dateien im lokalen Benutzerprofil.
- Das Rechnungsarchiv erkennt lokale Veränderungen über verkettete SHA-256-Prüfsummen und optional Ed25519-Signaturen. Es ist **kein** Ersatz für revisionssichere Aufbewahrung oder eine qualifizierte elektronische Signatur.
- Der lokale Archivschlüssel ist unter Windows per DPAPI an das Benutzerkonto gebunden. Sicherungen (`.erechnung`) sind mit `age` und einem Kennwort verschlüsselt.
- Deep Links akzeptieren nur `erechnung-review://print-job/<UUID>`. Der Diagnosebericht wird serverseitig erzeugt und enthält keine Rechnungsinhalte oder persönlichen Pfade.

Besonders interessiert sind wir an Problemen wie Pfad-Traversal, unsicherer Deep-Link-Verarbeitung, Manipulationen am Archiv, die die Prüfung nicht erkennt, Schwächen bei Sicherung und Wiederherstellung sowie Schwachstellen in der Installations- und Signaturkette.

Das in den Skripten verwendete Entwicklungszertifikat `CN=ERechnung Development` samt Passwort `ERechnung-Dev-Only` ist absichtlich öffentlich. Es ist nur für Test- und Vorführbuilds gedacht; die veröffentlichten Setups sind derzeit nicht vertrauenswürdig signiert. Produktionsbuilds (`-SigningMode Production`) lehnen selbstsignierte Pakete ab.

Details zur Architektur: [docs/architecture.md](docs/architecture.md).
