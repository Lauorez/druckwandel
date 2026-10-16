# Druckwandel mit Slint: erster Implementierungsschritt

Die neue Desktop-Anwendung liegt getrennt von der bisherigen Tauri-Version. Sie importiert PDFs bis 100 MB, liest den enthaltenen Text und speichert Original und Rechnungsentwurf lokal. Unter Windows übernimmt „Druckeingang“ Aufträge der vorhandenen Druckbrücke, einschließlich Metadaten und Bestätigung; der Link `erechnung-review://print-job/<UUID>` wird als Startargument verarbeitet. Eine einfache Erfassungsmaske für eine Rechnungsposition kann XRechnung-XML erzeugen. Die Ausgabe wird vor dem Speichern durch KoSIT geprüft. Danach kann die unveränderte Datei über die Systemfreigabe an ein Mailprogramm übergeben werden. Ein PDF-Pfad kann auch als Startargument übergeben werden.

```sh
cargo run -p druckwandel-native --locked
cargo test -p druckwandel-native --locked
cargo test -p druckwandel-native --test kosit -- --ignored
```

Für einen Prüflauf aus einem vollständigen JSON-Entwurf:

```sh
cargo run -p druckwandel-native --bin druckwandel-xml --locked -- apps/native/tests/fixtures/standard-invoice.json target/rechnung.xml
```

Die Oberfläche unterstützt noch keine automatische Felderkennung, mehr als eine bearbeitbare Position, Sonderfälle oder ZUGFeRD. Die Windows-Protokollregistrierung und ein nativer Installer fehlen noch; neue Aufträge lassen sich über „Druckeingang“ manuell abrufen. Der KoSIT-Integrationstest benötigt das lokale Prüfpaket aus `npm run validators:fetch`. Bis die fehlenden Funktionen portiert und auf beiden Plattformen geprüft sind, bleibt die bisherige Tauri-Anwendung der nutzbare Stand.
