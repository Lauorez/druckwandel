import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "..");

describe("gemeinsamer Windows-Installer", () => {
  it("bindet den Drucker in den Tauri-NSIS-Installer ein", () => {
    const config = JSON.parse(
      readFileSync(resolve(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(config.bundle.windows.nsis.installerHooks).toBe("nsis/installer-hooks.nsh");

    const hooks = readFileSync(
      resolve(root, "apps/desktop/src-tauri/nsis/installer-hooks.nsh"),
      "utf8",
    );
    expect(hooks).toContain("NSIS_HOOK_PREINSTALL");
    expect(hooks).toContain("NSIS_HOOK_PREUNINSTALL");
    expect(hooks).toContain("CurrentBuildNumber");
    expect(hooks).toContain("$WINDIR\\Sysnative\\WindowsPowerShell");
    expect(hooks).toContain("Printer.msix");
    expect(hooks).toContain("$UpdateMode <> 1");
    expect(hooks).toContain("SetOutPath $INSTDIR");
  });

  it("installiert ausschließlich das erwartete, signierte Druckerpaket", () => {
    const installer = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/InstallPrinter.ps1"),
      "utf8",
    );
    expect(installer).toContain('$packageName = "ERechnung.VirtualPrinter.PoC"');
    expect(installer).toContain("Get-AuthenticodeSignature");
    expect(installer).toContain("Test-CertificateTrusted");
    expect(installer).toContain("Add-AppxPackage");
    expect(installer).toContain("Wait-ForPrinter");
  });

  it("verhindert einen unsignierten Produktionsbuild", () => {
    const buildScript = readFileSync(
      resolve(root, "scripts/build-windows-installer.ps1"),
      "utf8",
    );
    expect(buildScript).toContain('$SigningMode -eq "Production"');
    expect(buildScript).toContain("SignerCertificate.Subject -eq");
    expect(buildScript).toContain("$installerSignature.Status -ne \"Valid\"");
  });
});
