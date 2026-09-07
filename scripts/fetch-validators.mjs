#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { createWriteStream } from "node:fs";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "apps/desktop/src-tauri/resources/validators");
const manifest = JSON.parse(await readFile(join(root, "manifest.json"), "utf8"));

const artifacts = [
  {
    url: "https://github.com/itplr-kosit/validator/releases/download/v1.5.0/validator-1.5.0-standalone.jar",
    file: join(root, manifest.kosit.jar),
  },
  {
    url: "https://repo1.maven.org/maven2/org/mustangproject/Mustang-CLI/2.16.2/Mustang-CLI-2.16.2.jar",
    file: join(root, manifest.mustang.jar),
  },
];

async function download(url, file) {
  await mkdir(dirname(file), { recursive: true });
  const response = await fetch(url);
  if (!response.ok || !response.body) throw new Error(`${url}: HTTP ${response.status}`);
  await pipeline(Readable.fromWeb(response.body), createWriteStream(file));
  const digest = createHash("sha256").update(await readFile(file)).digest("hex");
  console.log(`${digest}  ${file}`);
  return digest;
}

const checksums = {};
for (const artifact of artifacts) {
  checksums[artifact.file.replace(`${root}/`, "")] = await download(artifact.url, artifact.file);
}
await writeFile(join(root, "checksums.json"), `${JSON.stringify(checksums, null, 2)}\n`);
console.log("JARs geladen. XRechnung-Konfiguration und Temurin-JRE bitte gemäß README ins gleiche Paket legen.");
console.log("Die Anwendung startet Java nur aus jre/bin, niemals aus PATH.");
