# Umstieg auf Rust und Slint

Stand: 16.10.2026. Ausgangspunkt: Druckwandel 0.3.5.

Umsetzung begonnen: Die [neue Rust/Slint-App](../../apps/native/README.md) importiert PDFs einschließlich bestehender Windows-Druckaufträge, speichert Entwürfe und kann eine einfache XRechnung nach KoSIT-Prüfung ausgeben und per Systemfreigabe weitergeben. Der bisherige Stand ist im gesperrten Branch `legacy/tauri-0.3.5` eingefroren. `main` enthält ausschließlich Rust/Slint.

Druckwandel soll eine schlanke Desktop-Anwendung für Windows und macOS werden. Der Kern bleibt: PDF-Rechnungen übernehmen, Daten prüfen und korrigieren, XRechnung oder ZUGFeRD erzeugen und die fertige Datei speichern oder per E-Mail weitergeben. Die frühere Roadmap und Produktdokumentation bleiben im Legacy-Branch erhalten.

## Produktumfang

- **Behalten:** Windows-Druckeingang, PDF-Import auf beiden Plattformen, Vorschau und manuelle Korrektur, XRechnung, ZUGFeRD, lokale Entwürfe und Lernprofile.
- **Entfernen:** Archivfunktion, lokaler DATEV-Buchungsexport einschließlich EXTF und Belegpaket, Backup- und Diagnoseoberfläche. Es gibt noch keine Nutzerdaten, die aus dem Archiv migriert werden müssen.
- **Später ergänzen:** Direkter DATEV-Belegupload ohne Buchungssatz. Er gehört nicht zum ersten Slint-Release.

## Anwendung und Ausgabe

Die Oberfläche wird mit Slint erstellt; App-Zustand und Fachlogik laufen in Rust. React, TypeScript, Tauri und die zugehörige Node-Buildkette entfallen aus der ausgelieferten Anwendung. Die vorhandene .NET-NativeAOT-Komponente für den virtuellen Windows-Drucker sowie die unabhängigen Java-Prüfer bleiben externe Komponenten. Der Drucker übergibt PDFs weiterhin an den Eingang der App. Auf macOS ist PDF-Import vorgesehen, kein virtueller Drucker.

Extraktion, Rechnungsmodell, Betragsberechnung, Korrekturen, Lernregeln sowie XML- und PDF-Erzeugung werden nach Rust portiert. Die bisher unterstützten Rechnungsfälle sollen erhalten bleiben. Für ZUGFeRD muss die Ausgabe weiterhin ein gültiges PDF/A-3 mit eingebettetem XML sein. KoSIT, Mustang und veraPDF prüfen die tatsächlich erzeugten Dateien. Die Implementierung darf eine Rechnung erst als fertig anzeigen, wenn die Prüfung ihrer Ausgabebytes erfolgreich war.

Der bisher an das Archiv gekoppelte Abschluss wird durch **„Prüfen und speichern“** ersetzt. Nach Wahl von XRechnung oder ZUGFeRD erzeugt und validiert die App die Ausgabe und speichert sie am vom Nutzer gewählten Ort. **„Per E-Mail senden“ bleibt bis zu einem erfolgreichen Speichern deaktiviert.** Der Button öffnet über die Systemfreigabe einen Entwurf im installierten Mailprogramm und hängt genau die gespeicherte XML- oder PDF-Datei an. Der Nutzer versendet selbst; Druckwandel benötigt dafür weder SMTP noch ein Mailkonto. Vorgesehen sind das Windows Share Sheet und `NSSharingService` auf macOS. Falls kein geeignetes Mailziel verfügbar ist oder die Datei nicht mehr existiert, zeigt die App den Speicherort zum manuellen Anhängen an.

Eine Änderung am Entwurf oder ein Wechsel des Ausgabeformats hebt die E-Mail-Freigabe auf. Vor dem Teilen wird geprüft, ob die gespeicherte Datei noch der validierten Ausgabe entspricht. Eine erneute Prüfung und Speicherung schaltet den Button wieder frei. Eine interne Ablage fertiger Ausgaben oder ein neues Archiv wird nicht eingeführt.

## DATEV-Belegupload als zweite Phase

Für die spätere Anbindung ist der **DATEV Belegbilderservice Rechnungswesen** über `accounting:documents` vorgesehen. Er überträgt Dokumente nach DATEV Unternehmen online. Der DATEV Buchungsdatenservice (`accounting:extf-files`) ist für strukturierte Buchungsdaten gedacht und gehört nicht zu diesem Vorhaben. [DATEV-Schnittstellenvorgaben](https://developer.datev.de/de/guides/interface-requirements)

Phase 2 ergänzt DATEV-Anmeldung, Auswahl des berechtigten Bestands und einen bewussten Upload der bereits gespeicherten und validierten Datei. Es wird kein Buchungssatz erzeugt oder übertragen. Vor dem Bau der Funktion sind DATEV-Sandbox und API-Zugang einzurichten; für Produktion ist die technische Freigabe durch DATEV einzuplanen. Unterstützte Dateiformate und Bestandsversion werden vor der Übertragung geprüft. Veraltete Ablagemetadaten dürfen nicht unbeabsichtigt den Status „Gebucht“ auslösen. [DATEV API-Einstieg](https://developer.datev.de/de/product-detail/accounting-documents/2.0/documentation), [DATEV-Änderungshinweis vom 20.05.2026](https://developer.datev.de/de/news/details/mk4ag8vvheclwv04ywpzsopc)

## Auslieferung und Abnahme

- Windows- und macOS-Builds sowie Installer werden ohne Tauri erstellt und über GitHub Actions geprüft. Der Windows-Installer enthält weiterhin Druckerpaket und benötigte Prüfer; der macOS-Build enthält die dort benötigten Prüfer.
- Die bestehenden Testbelege und unterstützten Rechnungsfälle werden auf die Rust-Implementierung übertragen. XRechnung und ZUGFeRD müssen zusätzlich die unabhängigen Prüfer bestehen; bei ZUGFeRD wird das eingebettete XML mit der validierten XML-Ausgabe verglichen.
- Auf beiden Plattformen werden Import, Entwurfswiederaufnahme, Speichern, E-Mail-Anhang und die Sperre nach Änderungen geprüft. Unter Windows kommen Druckerübergabe sowie Installation und Deinstallation hinzu.
- RAM-Verbrauch wird für gleiche Nutzungsszenarien mit Version 0.3.5 verglichen und dokumentiert. Ein fester Grenzwert ist kein Release-Kriterium.

Der Quellcode bleibt unter MIT. Für die kostenlose Slint-Desktop-Lizenz erscheint die erforderliche Slint-Nennung in einem über das Hauptmenü erreichbaren „Über“-Dialog. [Slint-Lizenzbedingungen](https://slint.dev/terms-and-conditions)

## Ergänzung: lokale KI-Erkennung

Neben der Schemaerkennung wird eine lokale KI-Methode auswählbar vorbereitet. Das kleine LLM und sein spezieller LoRA-Adapter entstehen vollständig in einem separaten Repository. Hier werden ausschließlich der gemeinsame versionierte Erkennungsvertrag, die Anbieterregistrierung und die Auswahloberfläche vorbereitet. Ohne registrierten lokalen Adapter bleibt die KI-Methode ausdrücklich nicht verfügbar. Es gibt keine externe API. Details: [Erkennungsarchitektur](../recognition.md).
