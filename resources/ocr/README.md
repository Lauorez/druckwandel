# Offline-OCR

`python scripts/fetch-ocr.py` lädt das private Windows-x64-Paket und die festgelegten deutschen/englischen Sprachdaten mit SHA-256-Prüfung. Es verändert weder PATH noch die systemweite Installation. 7-Zip wird zum Entpacken benötigt.

Quellen: [Tesseract-Dokumentation](https://tesseract-ocr.github.io/tessdoc/Installation.html), [Windows-Build von UB Mannheim](https://github.com/UB-Mannheim/tesseract), [tessdata_fast](https://github.com/tesseract-ocr/tessdata_fast/tree/4.1.0). Laufzeit, DLLs, mitgelieferte Dokumentation und Sprachdaten werden ausschließlich beim Build bereitgestellt; Produktcode führt keine Downloads aus. Die Originalhinweise unter `doc/` werden mitgeliefert. Tesseract und tessdata_fast stehen unter Apache-2.0; die enthaltenen Bibliotheken behalten ihre eigenen Lizenzen. Das gebündelte Entwicklungsartefakt ersetzt keine Prüfung der Distributionsbedingungen für einen öffentlichen Release.

Auf macOS/Linux wird eine lokale Tesseract-Installation mit `deu`/`eng` verwendet. Eine private Laufzeit unter `Contents/Resources/ocr` wird ebenfalls erkannt. `DRUCKWANDEL_TESSERACT` kann für Entwicklung und Tests einen ausführbaren Pfad vorgeben.

Die App rendert höchstens eine Seite gleichzeitig (4 Millionen Pixel), verarbeitet höchstens 40 Seiten und beendet einen OCR-Prozess nach 30 Sekunden beziehungsweise den Dokumentlauf nach 120 Sekunden. PNG/TSV-Dateien liegen nur in einem kurzlebigen temporären Ordner. Original-PDFs werden nicht umgeschrieben. Erkannte Wörter erhalten PDF-Koordinaten und OCR-Kennzeichnungen; Vorschläge müssen manuell geprüft werden.
