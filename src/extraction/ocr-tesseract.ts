import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCanvas } from "@napi-rs/canvas";
import { getDocument } from "pdfjs-dist/legacy/build/pdf.mjs";
import type { DocumentPage, OcrAdapter, SourceToken } from "./types.js";

export interface TesseractCliOptions {
  executable?: string;
  languages?: string;
  scale?: number;
  minimumWordConfidence?: number;
}

function execute(command: string, args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { windowsHide: true });
    const output: Buffer[] = [];
    const errors: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => output.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => errors.push(chunk));
    child.on("error", (error) => reject(new Error(`OCR konnte nicht gestartet werden (${command}): ${error.message}`)));
    child.on("close", (code) => code === 0
      ? resolve(Buffer.concat(output).toString("utf8"))
      : reject(new Error(`Tesseract endete mit Code ${code}: ${Buffer.concat(errors).toString("utf8").trim()}`)));
  });
}

export function parseTesseractTsv(tsv: string, page: number, scale: number, minimumConfidence: number): SourceToken[] {
  const rows = tsv.split(/\r?\n/).slice(1);
  return rows.flatMap((row, index) => {
    const columns = row.split("\t");
    if (columns.length < 12 || columns[0] !== "5") return [];
    const confidence = Number.parseFloat(columns[10] ?? "-1");
    const text = columns.slice(11).join("\t").trim();
    if (!text || confidence < minimumConfidence) return [];
    const left = Number.parseFloat(columns[6] ?? "0") / scale;
    const top = Number.parseFloat(columns[7] ?? "0") / scale;
    const width = Number.parseFloat(columns[8] ?? "0") / scale;
    const height = Number.parseFloat(columns[9] ?? "0") / scale;
    return [{ id: `p${page}-ocr${index}`, page, text, box: { x: left, y: top, width, height }, origin: "ocr" as const }];
  });
}

/** Fully local OCR adapter. Requires a locally installed Tesseract executable and language data. */
export class TesseractCliOcrAdapter implements OcrAdapter {
  readonly name = "Tesseract CLI";
  private readonly executable: string;
  private readonly languages: string;
  private readonly scale: number;
  private readonly minimumConfidence: number;

  constructor(options: TesseractCliOptions = {}) {
    this.executable = options.executable ?? "tesseract";
    this.languages = options.languages ?? "deu+eng";
    this.scale = options.scale ?? 2;
    this.minimumConfidence = options.minimumWordConfidence ?? 35;
  }

  async recognize(pdf: Uint8Array): Promise<DocumentPage[]> {
    const directory = await mkdtemp(join(tmpdir(), "erechnung-ocr-"));
    const task = getDocument({ data: Uint8Array.from(pdf), useWorkerFetch: false });
    const document = await task.promise;
    const pages: DocumentPage[] = [];
    try {
      for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber += 1) {
        const page = await document.getPage(pageNumber);
        const viewport = page.getViewport({ scale: this.scale });
        const canvas = createCanvas(Math.ceil(viewport.width), Math.ceil(viewport.height));
        const context = canvas.getContext("2d");
        await page.render({ canvas: canvas as never, canvasContext: context as never, viewport }).promise;
        const imagePath = join(directory, `page-${pageNumber}.png`);
        await writeFile(imagePath, await canvas.encode("png"));
        const tsv = await execute(this.executable, [imagePath, "stdout", "-l", this.languages, "tsv"]);
        pages.push({
          page: pageNumber,
          width: viewport.width / this.scale,
          height: viewport.height / this.scale,
          tokens: parseTesseractTsv(tsv, pageNumber, this.scale, this.minimumConfidence),
        });
        page.cleanup();
      }
      return pages;
    } finally {
      await task.destroy();
      await rm(directory, { recursive: true, force: true });
    }
  }
}
