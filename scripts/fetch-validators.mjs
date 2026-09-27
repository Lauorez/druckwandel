#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rm, cp, writeFile } from "node:fs/promises";
import { createWriteStream, existsSync } from "node:fs";
import { basename, dirname, join, relative, resolve, isAbsolute } from "node:path";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "apps/desktop/src-tauri/resources/validators");
const manifest = JSON.parse(await readFile(join(root, "manifest.json"), "utf8"));

async function removeGeneratedDirectory(path, parent) {
  const target = resolve(path);
  const within = relative(resolve(parent), target);
  if (!within || within === ".." || within.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`) || isAbsolute(within)) {
    throw new Error(`Unsicheres Löschziel: ${target}`);
  }
  await rm(target, { recursive: true, force: true });
}

async function download(url, file) {
  await mkdir(dirname(file), { recursive: true });
  const response = await fetch(url, { headers: { "User-Agent": "erechnungs-assistent-validators" } });
  if (!response.ok || !response.body) throw new Error(`${url}: HTTP ${response.status}`);
  await pipeline(Readable.fromWeb(response.body), createWriteStream(file));
  const digest = createHash("sha256").update(await readFile(file)).digest("hex");
  console.log(`${digest}  ${file}`);
  return digest;
}

async function findFile(directory, name) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isFile() && entry.name === name) return path;
    if (entry.isDirectory()) {
      const nested = await findFile(path, name);
      if (nested) return nested;
    }
  }
  return undefined;
}

function powershellQuote(value) {
  return `'${String(value).replace(/'/g, "''")}'`;
}

function extractArchive(archive, dest) {
  if (process.platform === "win32") {
    const result = spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        `Expand-Archive -LiteralPath ${powershellQuote(archive)} -DestinationPath ${powershellQuote(dest)} -Force`,
      ],
      { stdio: "inherit", windowsHide: true },
    );
    if (result.status !== 0) {
      throw new Error(`Archiv konnte nicht entpackt werden: ${archive}`);
    }
    return;
  }

  const gzip = /\.(tar\.gz|tgz)$/i.test(archive);
  const result = spawnSync("tar", [gzip ? "-xzf" : "-xf", basename(archive), "-C", dest], {
    cwd: dirname(archive),
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw new Error(`Archiv konnte nicht entpackt werden: ${archive}`);
  }
}

const artifacts = [
  {
    url: `https://github.com/itplr-kosit/validator/releases/download/v${manifest.kosit.engineVersion}/${basename(manifest.kosit.jar)}`,
    file: join(root, manifest.kosit.jar),
  },
  {
    url: `https://repo.maven.apache.org/maven2/org/mustangproject/Mustang-CLI/${manifest.mustang.engineVersion}/${basename(manifest.mustang.jar)}`,
    file: join(root, manifest.mustang.jar),
  },
  {
    url: `https://repo.maven.apache.org/maven2/org/verapdf/apps/greenfield-apps/${manifest.verapdf.engineVersion}/${basename(manifest.verapdf.jar)}`,
    file: join(root, manifest.verapdf.jar),
  },
];

const checksums = {};
for (const artifact of artifacts) {
  checksums[relative(root, artifact.file).replaceAll("\\", "/")] = await download(artifact.url, artifact.file);
}

const release = await fetch(
  `https://api.github.com/repos/itplr-kosit/validator-configuration-xrechnung/releases/tags/${manifest.kosit.configurationTag}`,
  { headers: { "User-Agent": "erechnungs-assistent-validators", Accept: "application/vnd.github+json" } },
);
if (!release.ok) throw new Error(`XRechnung-Konfiguration: HTTP ${release.status}`);
const assets = await release.json();
const zipAsset = (assets.assets ?? []).find((asset) => /\.zip$/i.test(asset.name));
if (!zipAsset?.browser_download_url) throw new Error("XRechnung-Konfiguration: ZIP nicht gefunden.");
const zipPath = join(tmpdir(), zipAsset.name);
checksums[`kosit/${zipAsset.name}`] = await download(zipAsset.browser_download_url, zipPath);
const extractDir = join(tmpdir(), `xrechnung-config-${Date.now()}`);
await mkdir(extractDir, { recursive: true });
extractArchive(zipPath, extractDir);
const scenarios = await findFile(extractDir, "scenarios.xml");
if (!scenarios) throw new Error("scenarios.xml fehlt in der XRechnung-Konfiguration.");
const configRoot = dirname(scenarios);
const target = join(root, "kosit", "xrechnung");
await removeGeneratedDirectory(target, root);
await cp(configRoot, target, { recursive: true });
await removeGeneratedDirectory(extractDir, tmpdir());
console.log(`XRechnung-Konfiguration nach ${target}`);

const platform = process.platform === "darwin" ? "mac" : process.platform === "win32" ? "windows" : "linux";
const arch = process.arch === "arm64" ? "aarch64" : "x64";
const jreArchive = join(tmpdir(), platform === "windows" ? "temurin-jre.zip" : "temurin-jre.tar.gz");
const jreUrl = `https://api.adoptium.net/v3/binary/latest/21/ga/${platform}/${arch}/jre/hotspot/normal/eclipse?project=jdk`;
checksums[`jre/${platform}-${arch}`] = await download(jreUrl, jreArchive);
const jreExtract = join(tmpdir(), `temurin-jre-${Date.now()}`);
await mkdir(jreExtract, { recursive: true });
extractArchive(jreArchive, jreExtract);
const javaName = platform === "windows" ? "java.exe" : "java";
const javaPath = await findFile(jreExtract, javaName);
if (!javaPath) throw new Error("java fehlt in der geladenen JRE.");
let jreHome = dirname(dirname(javaPath));
if (existsSync(join(jreHome, "Home"))) jreHome = join(jreHome, "Home");
const jreTarget = join(root, "jre");
await removeGeneratedDirectory(jreTarget, root);
await cp(jreHome, jreTarget, { recursive: true });
await removeGeneratedDirectory(jreExtract, tmpdir());
console.log(`JRE nach ${jreTarget}`);

await writeFile(join(root, "checksums.json"), `${JSON.stringify(checksums, null, 2)}\n`);
console.log("Prüfpaket vollständig. Die Anwendung startet Java nur aus jre/bin, niemals aus PATH.");
