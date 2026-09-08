import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import type { ValidationResult } from "../domain/types.js";
import {
  evaluateKositOutcome,
  evaluateMustangOutcome,
  evaluateVeraPdfOutcome,
  type OfficialEngine,
  type OfficialValidationReport,
  type ProcessOutcome,
} from "./validation-report.js";

export interface ArtifactValidator {
  readonly name: string;
  readonly engine?: OfficialEngine;
  validate(path: string): Promise<ValidationResult>;
}

const DEFAULT_TIMEOUT_MS = 120_000;
const MAX_OUTPUT_BYTES = 8 * 1024 * 1024;

export function sha256Hex(contents: Uint8Array | string): string {
  return createHash("sha256").update(contents).digest("hex");
}

async function run(command: string, args: string[], timeoutMs = DEFAULT_TIMEOUT_MS): Promise<ProcessOutcome> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { windowsHide: true, env: { ...process.env, JAVA_TOOL_OPTIONS: undefined } });
    let output = "";
    let outputBytes = 0;
    const append = (chunk: Buffer) => {
      outputBytes += chunk.length;
      if (outputBytes <= MAX_OUTPUT_BYTES) output += chunk.toString("utf8");
    };
    const timer = setTimeout(() => {
      child.kill();
      resolve({ code: -1, output, timedOut: true });
    }, timeoutMs);
    child.stdout.on("data", append);
    child.stderr.on("data", append);
    child.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code: code ?? -1, output: outputBytes > MAX_OUTPUT_BYTES ? `${output}\n[truncated]` : output });
    });
  });
}

const JAVA_OFFLINE_FLAGS = [
  "-Djava.awt.headless=true",
  "-Djava.net.useSystemProxies=false",
  "-Dhttp.proxyHost=127.0.0.1",
  "-Dhttp.proxyPort=9",
  "-Dhttps.proxyHost=127.0.0.1",
  "-Dhttps.proxyPort=9",
  "-Djavax.xml.accessExternalDTD=",
  "-Djavax.xml.accessExternalSchema=",
  "-Djavax.xml.accessExternalStylesheet=",
];

async function readKositReport(reportDir: string, invoicePath: string): Promise<string | undefined> {
  const stem = basename(invoicePath).replace(/\.xml$/i, "");
  const reportFile = (await readdir(reportDir)).find((file) => file.toLowerCase().endsWith("-report.xml") && file.startsWith(stem))
    ?? (await readdir(reportDir)).find((file) => file.toLowerCase().endsWith("-report.xml"));
  if (!reportFile) return undefined;
  return readFile(join(reportDir, reportFile), "utf8");
}

export class KositValidator implements ArtifactValidator {
  readonly name = "KoSIT XRechnung";
  readonly engine = "kosit" as const;
  constructor(
    private readonly jar: string,
    private readonly scenarios: string,
    private readonly repository = dirname(scenarios),
    private readonly reportDirectory?: string,
    private readonly java = "java",
    private readonly ruleVersion = "xrechnung-3.0.2-2026-01-31",
  ) {}

  async validate(path: string): Promise<ValidationResult> {
    const report = await this.inspect(path);
    return { valid: report.valid, issues: report.issues };
  }

  async inspect(path: string): Promise<OfficialValidationReport> {
    const reportDir = this.reportDirectory ?? await mkdtemp(join(tmpdir(), "erechnung-kosit-"));
    await mkdir(reportDir, { recursive: true });
    try {
      const outcome = await run(this.java, [
        ...JAVA_OFFLINE_FLAGS,
        "-jar",
        this.jar,
        "-s",
        this.scenarios,
        "-r",
        this.repository,
        "-o",
        reportDir,
        path,
      ]);
      try {
        const reportXml = await readKositReport(reportDir, path);
        if (reportXml) outcome.reportXml = reportXml;
      } catch {
        /* missing report is evaluated below */
      }
      const evaluated = evaluateKositOutcome(outcome, this.ruleVersion);
      const xml = await readFile(path);
      return {
        ...evaluated,
        xmlSha256: sha256Hex(xml),
        pdfSha256: "",
        ...(outcome.reportXml ? { reportSha256: sha256Hex(outcome.reportXml) } : {}),
      };
    } finally {
      if (!this.reportDirectory) await rm(reportDir, { recursive: true, force: true });
    }
  }
}

export class VeraPdfValidator implements ArtifactValidator {
  readonly name = "veraPDF PDF/A";
  readonly engine = "verapdf" as const;
  constructor(private readonly executable = "verapdf", private readonly java?: string, private readonly jar?: string, private readonly ruleVersion = "pdfa-3b") {}
  async validate(path: string): Promise<ValidationResult> {
    const outcome = this.jar && this.java
      ? await run(this.java, [...JAVA_OFFLINE_FLAGS, "-jar", this.jar, "--flavour", "3b", "--format", "xml", "--maxfailures", "20", path])
      : await run(this.executable, ["--flavour", "3b", "--format", "xml", "--maxfailures", "20", path]);
    const reportXml = outcome.output.match(/<\?xml[\s\S]*<\/report>/i)?.[0] ?? outcome.output.match(/<report[\s\S]*<\/report>/i)?.[0];
    if (reportXml) outcome.reportXml = reportXml;
    const evaluated = evaluateVeraPdfOutcome(outcome, this.ruleVersion);
    return { valid: evaluated.valid, issues: evaluated.issues };
  }
}

export class MustangValidator implements ArtifactValidator {
  readonly name = "Mustang Factur-X/ZUGFeRD";
  readonly engine = "mustang" as const;
  constructor(private readonly jar: string, private readonly java = "java", private readonly ruleVersion = "factur-x-en16931") {}

  async validate(path: string): Promise<ValidationResult> {
    const report = await this.inspect(path);
    return { valid: report.valid, issues: report.issues };
  }

  async inspect(path: string): Promise<OfficialValidationReport> {
    const outcome = await run(this.java, [
      ...JAVA_OFFLINE_FLAGS,
      "-jar",
      this.jar,
      "--action",
      "validate",
      "--source",
      path,
      "--disable-file-logging",
    ]);
    const xmlMatch = outcome.output.match(/<\?xml[\s\S]*<\/validation>/i) ?? outcome.output.match(/<\s*validation[\s\S]*<\/validation>/i);
    if (xmlMatch?.[0]) outcome.reportXml = xmlMatch[0];
    const evaluated = evaluateMustangOutcome(outcome, this.ruleVersion);
    const xml = await readFile(path);
    return {
      ...evaluated,
      xmlSha256: sha256Hex(xml),
      pdfSha256: "",
      ...(outcome.reportXml ? { reportSha256: sha256Hex(outcome.reportXml) } : {}),
    };
  }
}

export async function validateBytes(validator: ArtifactValidator, bytes: Uint8Array, extension: string): Promise<ValidationResult> {
  const dir = await mkdtemp(join(tmpdir(), "erechnung-artifact-"));
  const path = join(dir, `invoice.${extension}`);
  try {
    await writeFile(path, bytes);
    return await validator.validate(path);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
