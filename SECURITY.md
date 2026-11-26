# Sicherheitsmeldungen

Sicherheitslücken bitte über die private GitHub-Sicherheitsmeldung dieses Repositorys melden. Keine echten Rechnungen oder personenbezogenen Daten in öffentliche Issues stellen.

Bitte Commit oder Version, Betriebssystem, betroffene Komponente und reproduzierbare Schritte mit künstlichen Testdaten angeben.

Dateipfade, SQLite, Druckübergabe und Starts gebündelter Prüfer werden in Rust verarbeitet. Die Druckbrücke ist ein separates .NET-Projekt. Erkannte Rechnungsdaten bleiben Vorschläge; nur erfolgreich unabhängig geprüfte Ausgabebytes dürfen als fertige Datei gespeichert und geteilt werden.

Die geplante lokale KI ist ein separates Projekt und noch nicht enthalten; ein lokaler KI-Adapter darf Rechnungsdaten nicht an externe Dienste übertragen. Die optionale Cloud-Erkennung über W System ist standardmäßig inaktiv, erfordert eine ausdrückliche Freigabe und speichert den Nutzerschlüssel im System-Schlüsselbund. Die Auto-Update-Prüfung ruft ausschließlich anonym per HTTPS das Ed25519-signierte Manifest (`latest.json` + `latest.json.sig`) von GitHub Releases ab und prüft heruntergeladene Installer vor dem Start per SHA-256 und Dateigröße.
