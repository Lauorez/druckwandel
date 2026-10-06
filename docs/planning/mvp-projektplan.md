# E‑Rechnung‑MVP – Projektplan
**Stand:** 20.08.2026
**Arbeitstitel:** E‑Rechnung‑MVP

## 1. Produktthese

### Problem
Ab 2028 endet für inländische B2B-Umsätze grundsätzlich die umsatzabhängige Übergangsfrist für die deutsche E‑Rechnung. Viele kleine Betriebe arbeiten heute trotzdem noch mit Word, Excel, älteren Faktura-/Handwerksprogrammen oder individuell gewachsenen Abläufen.

Die eigentliche Hürde ist für diese Zielgruppe nicht „eine Rechnung schreiben“, sondern:

- strukturierte EN‑16931-Daten korrekt erzeugen,
- ZUGFeRD/Factur‑X bzw. XRechnung korrekt ausgeben,
- Pflichtfelder und steuerliche Sonderfälle beherrschen,
- technisch valide Dateien erzeugen,
- ohne den vorhandenen Arbeitsablauf vollständig zu ersetzen.

### Produktversprechen
> **Behalte deinen bisherigen Rechnungsworkflow – wir machen daraus eine gültige E‑Rechnung.**

Das Produkt ist zunächst **keine Buchhaltungssoftware** und **kein ERP**, sondern eine Compliance- und Konvertierungsschicht zwischen dem bestehenden Rechnungsprozess und der E‑Rechnung.

## 2. Zentrales Produktmerkmal: virtueller PDF-Drucker

Der langfristig wichtigste Einstiegspunkt ist ein virtueller Windows-Drucker:

```text
Word / Excel / Legacy-ERP / Handwerkersoftware
                    │
                    ▼
          Drucken → „E-Rechnung“
                    │
                    ▼
             PDF/Print-Output
                    │
                    ▼
           Datenextraktion
                    │
                    ▼
       Prüfung + Ergänzung durch Nutzer
                    │
                    ▼
          kanonisches Invoice-Modell
                    │
              ┌─────┴─────┐
              ▼           ▼
         ZUGFeRD       XRechnung
         PDF/A-3          XML
```

### UX-Ziel
Der Anwender soll im Normalfall nur:

1. seine Rechnung wie bisher erstellen,
2. „E‑Rechnung“ als Drucker auswählen,
3. erkannte Daten kurz prüfen,
4. auf **Erstellen/Senden** klicken.

Begriffe wie CII, UBL, Schematron oder BT‑xxx sollen im normalen Workflow nicht sichtbar sein.

## 3. MVP-Strategie

Der virtuelle Drucker ist das zentrale Differenzierungsmerkmal, aber **nicht der erste technische Baustein**.

### Phase 0 – technische Machbarkeit
Ein kleiner Proof of Concept beantwortet zuerst:

- Kann aus beliebigen digital erzeugten PDF-Rechnungen genügend Text und Layoutinformation stabil extrahiert werden?
- Wie zuverlässig lassen sich Rechnungskopf, Kunde, Rechnungsnummer, Daten, Positionen, Steuern und Summen erkennen?
- Welche Angaben müssen fast immer manuell ergänzt werden?
- Kann aus einem beliebigen Input-PDF zuverlässig ein valides ZUGFeRD-PDF/A‑3 erzeugt werden?
- Welche Validator-Kombination liefert reproduzierbare Ergebnisse?

### Phase 1 – MVP: PDF rein → E‑Rechnung raus
Noch ohne virtuellen Drucker.

**Input**
- PDF per Drag & Drop
- optional bereits strukturierte Rechnungsdaten

**Pipeline**
1. PDF analysieren
2. Rechnungsfelder extrahieren
3. Confidence je Feld berechnen
4. Nutzerreview
5. internes Invoice-Modell erzeugen
6. fachliche Plausibilitätsprüfungen
7. EN‑16931-Regeln prüfen
8. Ausgabe erzeugen
9. Ergebnis erneut validieren

**Output**
- ZUGFeRD/Factur‑X, Profil EN 16931
- XRechnung als zusätzliche Ausgabe
- Validierungsbericht

### Phase 2 – virtueller Windows-Drucker
Die Phase-1-Pipeline wird hinter einem Windows-Drucker versteckt.

Der Drucker erzeugt bzw. übernimmt den Print-Output und öffnet unmittelbar die Review-Oberfläche.

Für moderne Windows-Versionen sollte die aktuelle **Print Support Virtual Printer / Print Support App**-Architektur von Microsoft geprüft und bevorzugt werden, statt einen neuen Legacy-V3/V4-Treiber zu bauen. Das reduziert langfristig das Risiko durch Microsofts Abkehr von klassischen Drittanbieter-Druckertreibern.

### Phase 3 – „One click“
Erkennung pro Rechnungslayout lernen:

```text
„Müller GmbH – Layout 3“
→ Feldpositionen / Regeln bekannt
→ 99 % sicher
→ keine Review nötig
→ direkt erzeugen
```

Hier entsteht der eigentliche Lock-in: Wiederkehrende Rechnungen aus derselben Altsoftware werden zunehmend automatisch konvertiert.

## 4. Technische Architektur

### 4.1 Leitprinzip
**Standards niemals direkt in UI oder Extraktionslogik einbauen.**

```text
Input Layer
   │
   ├─ PDF Upload
   ├─ Virtual Printer
   └─ später API / Watch Folder
   │
   ▼
Extraction Layer
   │
   ▼
Canonical Invoice Model
   │
   ├─ Business Validation
   ├─ EN-16931 Mapping
   └─ Tax/Total Validation
   │
   ▼
Format Adapters
   ├─ ZUGFeRD / Factur-X (CII + PDF/A-3)
   └─ XRechnung (UBL/CII)
   │
   ▼
Independent Validators
```

### 4.2 Kanonisches Datenmodell
Beispiel:

```ts
interface Invoice {
  invoiceNumber: string;
  invoiceType: string;
  issueDate: string;
  serviceDate?: string;
  dueDate?: string;
  currency: string;

  seller: Party;
  buyer: Party;

  lines: InvoiceLine[];
  allowancesCharges: AllowanceCharge[];
  taxes: TaxBreakdown[];

  totals: {
    lineNet: Decimal;
    taxExclusive: Decimal;
    taxTotal: Decimal;
    taxInclusive: Decimal;
    payable: Decimal;
  };

  payment: PaymentInfo;
  references?: InvoiceReferences;
}
```

**Wichtig:** Geldbeträge nicht mit binären Floating-Point-Werten berechnen. Decimal-/Festkommaarithmetik verwenden.

### 4.3 Extraktionspipeline
Priorität:

1. **PDF-Textlayer** und Positionen auslesen
2. regelbasierte Erkennung
3. layoutbasierte Templates für bekannte Absender
4. optional lokales ML/LLM zur semantischen Zuordnung
5. OCR nur für gescannte PDFs als Fallback

Die KI darf **keine Beträge erfinden**. Jedes extrahierte Feld erhält:
- Wert
- Quelle im Dokument
- Confidence
- Transformationshistorie

Bei Summen und Steuerwerten wird unabhängig nachgerechnet.

### 4.4 Validierung
Validierung muss mehrstufig erfolgen:

**A. Datentypen / Schema**
- XML Schema

**B. Geschäftsregeln**
- Schematron / EN‑16931
- XRechnung-spezifische Regeln

**C. PDF**
- PDF/A‑3-Konformität
- eingebettete XML korrekt referenziert

**D. Eigene Plausibilitätsprüfung**
- Netto + Steuer = Brutto
- Positionssummen = Rechnungssummen
- Steuersätze passen zu Steuerbeträgen
- Pflichtfelder je Rechnungstyp vorhanden

Wichtig: Ein einzelner Drittanbieter-Validator wird nicht zur alleinigen Wahrheit. Die Validierungskomponente muss austauschbar sein.

### 4.5 Standard-Engine
Standards als versionierte Pakete:

```text
standards/
  en16931/
  zugferd/
    2.x/
  xrechnung/
    3.x/
    4.x/
```

Regel-/Standardupdates sollen möglichst ohne komplettes App-Upgrade ausgeliefert werden können.

### 4.6 Technologie-Vorschlag

**Windows-first Desktop-App**
- UI: React + TypeScript
- Desktop Shell: Electron oder Tauri
- lokaler Hintergrunddienst: getrennte Komponente
- Datenbank: SQLite
- Secrets: Windows Credential Manager / DPAPI
- lokale Dateien: verschlüsselte App-Datenablage optional

**Print-Komponente**
- bevorzugt Microsoft Print Support Virtual Printer / PSA evaluieren
- ggf. kleiner nativer Windows-Helper in C#/.NET

**E-Rechnungs-Engine**
Nicht neu erfinden, was bereits als etablierte Open-Source-Komponente existiert:
- offizielle XSD/Schematron-Artefakte verwenden
- KoSIT-Validator für XRechnung in Test/CI einbeziehen
- Mustangproject als Referenz bzw. mögliche Engine/Validator-Komponente evaluieren
- veraPDF bzw. äquivalente PDF/A-Prüfung einplanen

Die Engine bleibt hinter einer internen Schnittstelle, damit sie später ersetzt werden kann.

## 5. Local-first als Positionierung

Für die erste Zielgruppe ist **Local-first** attraktiv:

- Rechnungen enthalten sensible Geschäfts- und Kundendaten.
- Ein Handwerksbetrieb braucht für das reine Erzeugen einer E-Rechnung keine Cloud.
- Keine laufenden Serverkosten für die Basiskonvertierung.
- Einmalige Lizenz wird möglich.
- Gute Abgrenzung zu klassischen SaaS-Buchhaltungsprodukten.

Cloud-Funktionen später optional:
- E-Mail-Versand
- Peppol
- Multi-Device-Sync
- Backup
- Teamfunktionen
- zentrale Standardupdates
- Steuerberaterportal

## 6. Zielgruppe

### Primärer Beachhead-Markt
Kleine deutsche B2B-Unternehmen, die:
- keine moderne Faktura-/Buchhaltungssoftware einsetzen,
- ihre vorhandene Software nicht wechseln wollen,
- Rechnungen über Word/Excel/Legacy-Anwendungen erzeugen,
- relativ geringe Rechnungsvolumina haben,
- möglichst wenig Prozessänderung wollen.

Besonders interessant:
- Handwerk
- Montage-/Servicebetriebe
- kleine Bau-/Baunebengewerbe
- technische Dienstleister
- kleine Agenturen
- kleine Groß-/Fachhändler mit Altsoftware
- Freiberufler mit B2B-Kunden

### Nicht primär
- Unternehmen mit vollständig integriertem DATEV-/SAP-/ERP-E-Invoicing
- Konzerne
- Unternehmen mit komplexen EDI-Prozessen
- reine B2C-Betriebe

## 7. Wettbewerb und Positionierung

Große Anbieter wie Lexware Office und sevdesk unterstützen E‑Rechnungen bereits. Gegen sie sollte das Produkt **nicht** als „noch eine Buchhaltungssoftware“ antreten.

### Unser Wettbewerbswinkel

| Klassische Buchhaltung | E‑Rechnung‑MVP |
|---|---|
| Nutzer migriert seinen Rechnungsprozess | bestehender Prozess bleibt |
| Rechnung wird im neuen System erstellt | Rechnung kann weiter in alter Software entstehen |
| Cloud/SaaS oft zentral | local-first möglich |
| viele Buchhaltungsfunktionen | fokussierte E‑Rechnungs-Compliance |
| höherer Wechselaufwand | geringer Wechselaufwand |

**Kernbotschaft:**
> „Du musst nicht lernen, wie E‑Rechnung funktioniert. Drucke deine Rechnung einfach auf unseren Drucker.“

## 8. Monetarisierung

### Hypothese A – Desktop-Lizenz
Sehr passend zur Legacy-/Local-first-Zielgruppe.

Beispiel:
- 49–99 € einmalig pro Arbeitsplatz
- 12 Monate Standardupdates inklusive
- optional 20–40 €/Jahr für weitere Standard-/Featureupdates

Vorteile:
- leicht verständlich
- kein SaaS-Widerstand
- passt zum lokalen Produkt

Risiko:
- laufende regulatorische Pflege verursacht wiederkehrende Kosten.

### Hypothese B – niedriger SaaS-/Update-Tarif
Beispiel:
- Free: wenige Rechnungen/Monat
- Solo: 5–8 €/Monat
- Betrieb: 10–15 €/Monat

Vorteil:
- wiederkehrender Umsatz
- Standardupdates wirtschaftlich abgedeckt

### Wahrscheinlich sinnvoll: Hybrid
**Desktop-Basis + Update-/Cloud-Service.**

Beispiel:
- App-Kauf oder kostenlose Basis
- Standardupdates + automatische Erkennung + Cloud-Versand als günstiges Abo

Keine endgültige Preisentscheidung vor Nutzertests.

## 9. Marktlogik

Deutschland besitzt mehrere Millionen KMU; mehr als 99 % der Unternehmen fallen nach aktuellen IfM-/Destatis-Zahlen in diese Größenklasse. Das ist **nicht** der direkt adressierbare Markt.

Für die Planung muss der Markt heruntergebrochen werden:

```text
alle Unternehmen
→ B2B-relevant
→ ab 2028 tatsächlich ausstellungspflichtig
→ nicht durch Ausnahmen erfasst
→ keine passende moderne Faktura vorhanden
→ nutzt PDF/Word/Excel/Legacy-Software
→ akzeptiert Desktop-/Konverterlösung
```

Der relevante Markt ist also wesentlich kleiner als „alle KMU“, aber immer noch potenziell groß.

## 10. Go-to-Market

### Phase A – Problemvalidierung
10–20 Interviews mit:
- Handwerksbetrieben
- kleinen B2B-Dienstleistern
- Steuerberatern/Buchhaltern

Fragen:
- Wie werden Rechnungen heute erzeugt?
- Welche Software?
- Wie viele Rechnungen/Monat?
- B2B-Anteil?
- Wissen sie bereits, was 2028 passiert?
- Würden sie Software wechseln?
- Was wäre ihnen „weiter wie bisher, nur E-Rechnung“ wert?
- Wer entscheidet über Softwarekäufe?
- Welche Sonderfälle kommen vor?

### Phase B – kostenloser Converter
Landingpage + lokale Beta:

> PDF rein → valide ZUGFeRD-Rechnung raus.

Das misst echte Nutzung besser als Umfragen.

### Phase C – PDF-Drucker
An die aktiven Converter-Nutzer verteilen.

Messgrößen:
- Rechnungen/Nutzer/Woche
- Anteil ohne manuelle Korrektur
- durchschnittliche Zahl korrigierter Felder
- häufigste Fehlerfelder
- Anzahl unterschiedlicher Ursprungsprogramme
- Conversion zu wiederkehrender Nutzung

### Phase D – Vertrieb
Geeignete Kanäle:
- Steuerberater und Buchhaltungsbüros
- Handwerks-/Branchenforen
- IHK/HWK-nahe Informationsumfelder
- SEO rund um „E-Rechnung aus Word/Excel“
- Softwarehäuser mit alter Branchen-/Fakturasoftware
- White-Label-/OEM-Modell für Legacy-Anbieter

Besonders interessant ist **B2B2B**:
Ein Hersteller alter Handwerkersoftware integriert den Converter, statt selbst EN‑16931/ZUGFeRD vollständig zu implementieren.

## 11. Produkt-Roadmap

### M0 – Forschungsprototyp
- PDF text/layout extraction
- manuelles Invoice-Modell
- ZUGFeRD-Ausgabe
- Validator-Pipeline
- Testkorpus

### M1 – Converter MVP
- Drag & Drop
- automatische Erkennung
- Review UI
- ZUGFeRD EN16931
- XRechnung
- lokale Historie
- Updatefähige Standards

### M2 – Virtual Printer Beta
- Windows-Drucker
- automatisches Öffnen des Review-Flows
- Ursprungsanwendung/Layout erkennen
- Templates je Layout

### M3 – produktionsreif
- Installer + Signierung
- Auto-Update
- robuste Fehlerdiagnose
- Audit Trail
- Backup/Export
- umfangreiche Steuersonderfälle
- Security Review

### M4 – Ausbau
- Watch Folder
- Kommandozeile/API
- E-Mail-Inbox
- E-Rechnung empfangen/rendern
- DATEV-/Steuerberaterexport
- Peppol optional
- OEM SDK

## 12. Was bewusst NICHT im MVP steckt

- vollständige Finanzbuchhaltung
- Bankkontenanbindung
- Lohnabrechnung
- CRM
- Lager/Warenwirtschaft
- Steuererklärungen
- umfassendes Mahnwesen
- eigenes Peppol Access Point
- Mobile Apps

Jedes dieser Themen verwässert zunächst den Kernnutzen.

## 13. Größte technische Risiken

### 1. Arbiträre PDF-Layouts
Das Hauptproblem ist nicht das XML, sondern aus einem visuell beliebigen Dokument sicher strukturierte Daten zu gewinnen.

**Mitigation:** Confidence-basierter Review und wiederverwendbare Layout-Templates.

### 2. Rechnungspositionen
Tabellen in PDFs können schwer zuverlässig rekonstruiert werden.

**Mitigation:** Bounding boxes + Tabellenheuristik + Nutzerreview; keine stillen KI-Schätzungen.

### 3. Steuerliche Sonderfälle
Reverse Charge, steuerfreie Umsätze, Gutschriften, Abschlags-/Schlussrechnungen etc.

**Mitigation:** MVP-Scope zunächst begrenzen und Sonderfälle explizit erkennen/ablehnen, bevor fehlerhafte Rechnungen erzeugt werden.

### 4. Standardänderungen
ZUGFeRD, XRechnung und EN 16931 entwickeln sich weiter.

**Mitigation:** versionierte Standard-Pakete und Adapter.

### 5. Virtueller Drucker
Windows-Integration, Packaging und Signierung können aufwendiger sein als die eigentliche App.

**Mitigation:** Converter zuerst; Drucker als separater Adapter.

## 14. Compliance- und Sicherheitsprinzipien

- Keine Behauptung „rechtssicher“ ohne definierte Prüfbasis.
- Technisch besser: „gegen Standard X/Y Version Z validiert“.
- Original-PDF niemals unbemerkt verändern.
- Konvertiertes Ergebnis eindeutig als neues Artefakt behandeln.
- Eingabedaten, Extraktion und Nutzeränderungen nachvollziehbar protokollieren.
- Local-first: keine Rechnungsdaten an externe KI-Dienste senden, sofern der Nutzer dies nicht explizit aktiviert.
- Testkorpus mit positiven und negativen Rechnungsfällen.
- Regressionstests bei jedem Standardupdate.

## 15. Definition of Done für das erste echte MVP

Das MVP ist fertig, wenn ein Nutzer:

1. eine normale digital erzeugte PDF-Rechnung hochladen kann,
2. alle relevanten erkannten Werte mit Quellenmarkierung sieht,
3. falsche/fehlende Werte einfach korrigieren kann,
4. eine ZUGFeRD/Factur‑X-Rechnung im EN‑16931-Profil erzeugen kann,
5. optional XRechnung erzeugen kann,
6. einen verständlichen Validierungsstatus bekommt,
7. alles ohne Cloudkonto lokal erledigen kann,
8. und dieselbe Rechnung durch unabhängige Referenzvalidatoren erfolgreich geprüft wird.

## 16. Erste konkrete Arbeitspakete

### WP1 – Invoice Domain
- internes Datenmodell
- Decimal-Regeln
- Steuer- und Summenlogik
- Mapping EN 16931

### WP2 – E‑Invoice Engine
- CII/ZUGFeRD-Generator
- XRechnung-Generator
- PDF/A‑3-Einbettung
- Validatoren
- Testfixtures

### WP3 – PDF Extraction
- Text + Bounding Boxes
- Feldklassifikation
- Tabellen-/Positionsrekonstruktion
- Confidence-Modell
- OCR-Fallback

### WP4 – Review UI
- PDF links
- erkannte Felder rechts
- Klick auf Feld hebt Quelle im PDF hervor
- Warnungen verständlich anzeigen

### WP5 – Windows Print Adapter
- Print Support Virtual Printer PoC
- Printjob → lokales Dokument → App
- Installer/Permissions/Signierung untersuchen

### WP6 – Product Validation
- Interviews
- 50–100 echte anonymisierte Rechnungsbeispiele
- Beta mit 5–10 Betrieben
- Metriken sammeln

## 17. Aktuelle externe Rahmenbedingungen

- Die deutsche Übergangsfrist für Rechnungsaussteller mit Vorjahresumsatz bis 800.000 € läuft bis Ende 2027; danach wird die E‑Rechnung für relevante inländische B2B-Umsätze grundsätzlich verpflichtend.
- ZUGFeRD basiert auf EN 16931 und kombiniert in der hybriden Ausprägung strukturierte Rechnungsdaten mit PDF/A‑3.
- XRechnung ist die deutsche CIUS der EN 16931.
- Die Standards entwickeln sich weiter; XRechnung 4.0 befindet sich 2026 bereits in Vorbereitung.
- ViDA führt ab 1. Juli 2030 für grenzüberschreitende B2B-Umsätze EU-weite Digital Reporting Requirements auf Basis verpflichtender E‑Rechnungen ein.
- Microsoft stellt für moderne Windows-Versionen eine Print Support Virtual Printer-Architektur bereit, die ohne klassische Legacy-Drittanbieter-Treiber auskommt.

### Quellenbasis
- Bundesministerium der Finanzen: FAQ zur obligatorischen E‑Rechnung; BMF-Schreiben vom 15.10.2025
- Forum elektronische Rechnung Deutschland (FeRD): ZUGFeRD/Factur‑X
- XStandards Einkauf / KoSIT: XRechnung und Validator-Konfiguration
- Europäische Kommission: VAT in the Digital Age (ViDA)
- Microsoft Learn: Print Support App / Print Support Virtual Printer
- IfM Bonn / Destatis: Unternehmens- und KMU-Statistiken
- Open-Source-Referenzen zur Evaluation: KoSIT Validator, Mustangproject, veraPDF

## 18. Projektentscheidungen, die vorerst als gesetzt gelten

1. **PDF-Drucker ist Kernfeature und Differenzierungsmerkmal.**
2. **Converter-Core wird vor dem Drucker gebaut.**
3. **Windows-first.**
4. **Local/offline-first.**
5. **ZUGFeRD/Factur‑X EN16931 ist der Default für normales B2B.**
6. **XRechnung wird zusätzlich unterstützt.**
7. **Kanonisches internes Rechnungsmodell statt formatabhängiger Businesslogik.**
8. **Keine vollständige Buchhaltungssoftware im MVP.**
9. **Fehler lieber sichtbar blockieren als still „korrigieren“.**
10. **Produktpositionierung: vorhandenen Workflow erhalten statt Softwaremigration erzwingen.**
