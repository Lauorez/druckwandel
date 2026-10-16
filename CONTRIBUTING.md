# Mitwirken

Entwickelt wird auf `main` in Rust und Slint. Der eingefrorene Legacy-Branch bleibt unverändert.

## Voraussetzungen und Prüfungen

Rust gemäß `rust-toolchain.toml`, Python 3.12+ für das Prüfpaket, auf Windows MSVC Build Tools und für Installer NSIS. Der Drucker benötigt zusätzlich .NET 10.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/fetch-validators.py
cargo test --locked --test external_validators -- --ignored
```

Änderungen an der Erkennung müssen Fundstellen und bestehende Korrekturen erhalten. Änderungen an Ausgaben müssen mit den unabhängigen Prüfern bestehen. Fachlogik und Dezimalarithmetik liegen in Rust; die Druckbrücke nimmt ausschließlich Druckaufträge an. Der spätere KI-Anbieter gehört in ein separates Repository und wird über den [Erkennungsvertrag](docs/recognition.md) ergänzt.

Keine echten Rechnungen, Modellgewichte, Zugangsdaten oder Zertifikatschlüssel einchecken. Künstliche Testbelege und Regressionstests sollen konkrete fachliche Fälle abdecken.
