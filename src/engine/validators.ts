import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import type { ValidationResult } from "../domain/types.js";

export interface ArtifactValidator {
  readonly name: string;
  validate(path: string): Promise<ValidationResult>;
}

async function run(command: string, args: string[]): Promise<{ code: number; output: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { windowsHide: true });
    let output = "";
    child.stdout.on("data", (chunk) => output += chunk);
    child.stderr.on("data", (chunk) => output += chunk);
    child.on("error", reject);
    child.on("close", (code) => resolve({ code: code ?? -1, output }));
  });
}

export class KositValidator implements ArtifactValidator {
  readonly name = "KoSIT XRechnung";
  constructor(private readonly jar: string, private readonly scenarios: string, private readonly repository = dirname(scenarios), private readonly reportDirectory?: string) {}
  async validate(path: string): Promise<ValidationResult> {
    const reportDir = this.reportDirectory ?? await mkdtemp(join(tmpdir(), "erechnung-kosit-"));
    await mkdir(reportDir, { recursive: true });
    try {
      const result = await run("java", ["-jar", this.jar, "-s", this.scenarios, "-r", this.repository, "-o", reportDir, "-h", path]);
      let report = "";
      try {
        const stem = basename(path).replace(/\.xml$/i, "");
        const reportFile = (await readdir(reportDir)).find((file) => file.toLowerCase().endsWith("-report.xml") && file.startsWith(stem));
        if (reportFile) report = await readFile(join(reportDir, reportFile), "utf8");
      } catch { /* command output is returned below */ }
      const rejected = /<rep:report[^>]*valid="false"/i.test(report) || /Acceptance\|Error[\s\S]*\|\s*REJECT/i.test(result.output);
      const failed = result.code !== 0 || rejected;
      return { valid: !failed, issues: failed ? [{ severity: "error", code: "KOSIT", path, message: report || result.output || "KoSIT meldet ein nicht akzeptables Dokument." }] : [] };
    } finally { if (!this.reportDirectory) await rm(reportDir, { recursive: true, force: true }); }
  }
}

export class VeraPdfValidator implements ArtifactValidator {
  readonly name = "veraPDF PDF/A";
  constructor(private readonly executable = "verapdf") {}
  async validate(path: string): Promise<ValidationResult> {
    const result = await run(this.executable, ["--format", "text", path]);
    const valid = result.code === 0 && /PASS/i.test(result.output) && !/FAIL/i.test(result.output);
    return { valid, issues: valid ? [] : [{ severity: "error", code: "VERAPDF", path, message: result.output.trim() || "PDF/A-Prüfung fehlgeschlagen." }] };
  }
}

export class MustangValidator implements ArtifactValidator {
  readonly name = "Mustang Factur-X/ZUGFeRD";
  constructor(private readonly jar: string) {}
  async validate(path: string): Promise<ValidationResult> {
    const result = await run("java", [
      "-jar",
      this.jar,
      "--action",
      "validate",
      "--source",
      path,
      "--disable-file-logging",
    ]);
    const summaries = [...result.output.matchAll(/<summary\s+status="(valid|invalid)"\s*\/>/gi)];
    const finalStatus = summaries.at(-1)?.[1]?.toLowerCase();
    const valid = result.code === 0 && finalStatus === "valid";
    return {
      valid,
      issues: valid ? [] : [{
        severity: "error",
        code: "MUSTANG",
        path,
        message: result.output.trim() || "Die Factur-X-/ZUGFeRD-Prüfung ist fehlgeschlagen.",
      }],
    };
  }
}

export async function validateBytes(validator: ArtifactValidator, bytes: Uint8Array, extension: string): Promise<ValidationResult> {
  const dir = await mkdtemp(join(tmpdir(), "erechnung-artifact-"));
  const path = join(dir, `invoice.${extension}`);
  try { await writeFile(path, bytes); return await validator.validate(path); }
  finally { await rm(dir, { recursive: true, force: true }); }
}
