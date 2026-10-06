import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "..");

describe("gemeinsamer Windows-Installer", () => {
  it("bindet den Drucker in den Tauri-NSIS-Installer ein", () => {
    const config = JSON.parse(
      readFileSync(resolve(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(config.bundle.windows.nsis.installMode).toBe("currentUser");
    expect(config.bundle.windows.webviewInstallMode.type).toBe("offlineInstaller");
    expect(config.bundle.windows.nsis.installerHooks).toBe("nsis/installer-hooks.nsh");

    const hooks = readFileSync(
      resolve(root, "apps/desktop/src-tauri/nsis/installer-hooks.nsh"),
      "utf8",
    );
    expect(hooks).toContain("NSIS_HOOK_PREINSTALL");
    expect(hooks).toContain("NSIS_HOOK_POSTINSTALL");
    expect(hooks).toContain("NSIS_HOOK_PREUNINSTALL");
    expect(hooks).toContain("CurrentBuildNumber");
    expect(hooks).toContain("$WINDIR\\Sysnative\\WindowsPowerShell");
    expect(hooks).toContain("Printer.msix");
    expect(hooks).toContain("TrustPrinterCertificate.ps1");
    expect(hooks).toContain("-CertificateTrustScriptPath");
    expect(hooks).toContain("UpdateGuard.ps1");
    expect(hooks).toContain("PrepareUpdate");
    expect(hooks).toContain("RecordInstalledVersion");
    expect(hooks).toContain("$UpdateMode = 1");
    expect(hooks).toContain("SetOutPath $INSTDIR");
    expect(hooks).toContain("Die vollständige Installation wird abgebrochen");
    expect(hooks).toContain("SetErrorLevel 1");
    expect(hooks).not.toContain("Die Anwendung wird trotzdem installiert");
  });

  it("ersetzt eine Installation unter dem früheren Namen E-Rechnungs-Assistent", () => {
    const config = JSON.parse(
      readFileSync(resolve(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(config.productName).toBe("Druckwandel");
    expect(config.identifier).toBe("de.erechnung.converter");
    expect(config.plugins["deep-link"].desktop.schemes).toEqual(["erechnung-review"]);

    const hooks = readFileSync(
      resolve(root, "apps/desktop/src-tauri/nsis/installer-hooks.nsh"),
      "utf8",
    );
    expect(hooks).toContain("CurrentVersion\\Uninstall\\E-Rechnungs-Assistent");
    expect(hooks).toContain("${OrIf} $R7 != \"\"");
    expect(hooks).toContain("/S /UPDATE _?=$R6");
    expect(hooks).toContain("$SMPROGRAMS\\E-Rechnungs-Assistent.lnk");

    const guard = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/UpdateGuard.ps1"),
      "utf8",
    );
    expect(guard).toContain('$name -eq "Druckwandel"');
    expect(guard).toContain("*E-Rechnungs-Assistent*");
  });

  it("installiert ausschließlich das erwartete, signierte Druckerpaket", () => {
    const installer = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/InstallPrinter.ps1"),
      "utf8",
    );
    expect(installer).toContain('$packageName = "ERechnung.VirtualPrinter.PoC"');
    expect(installer).toContain("Get-AuthenticodeSignature");
    expect(installer).toContain("Test-CertificateTrusted");
    expect(installer).toContain("Ensure-DevelopmentCertificateTrusted");
    expect(installer).toContain("Cert:\\LocalMachine\\TrustedPeople");
    expect(installer).not.toContain("Import-Certificate");
    expect(installer).toContain("-Verb RunAs");
    expect(installer).toContain("Add-AppxPackage");
    expect(installer).toContain("Wait-ForPrinter");

    const trust = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/TrustPrinterCertificate.ps1"),
      "utf8",
    );
    expect(trust).toContain('"ERechnung.VirtualPrinter.PoC"');
    expect(trust).toContain("$signature.SignerCertificate.Thumbprint");
    expect(trust).toContain("Cert:\\LocalMachine\\TrustedPeople");
    expect(trust).toContain("Import-Certificate");
  });

  it("richtet einen frischen Windows-Rechner mit einem Setup-Skript ein", () => {
    const setup = readFileSync(resolve(root, "scripts/setup-windows.ps1"), "utf8");
    expect(setup).toContain("26100");
    expect(setup).toContain("PROCESSOR_ARCHITEW6432");
    expect(setup).toContain("AMD64");
    expect(setup).toContain("Is64BitProcess");
    expect(setup).toContain("LOCALAPPDATA");
    expect(setup).toContain("rustup-init.exe");
    expect(setup).toContain("dotnet-install.ps1");
    expect(setup).not.toContain("-Verb RunAs");
    expect(setup).toContain("Expand-Archive");
    expect(setup).not.toContain("--force-local");
    expect(setup).toContain("create-dev-cert.ps1");
    expect(setup).toContain("validators:fetch");
    expect(setup).toContain("demo:invoice");
    expect(setup).toContain("build-windows-installer.ps1");
    expect(setup).toContain("java.exe");
  });

  it("verhindert einen unsignierten Produktionsbuild", () => {
    const buildScript = readFileSync(
      resolve(root, "scripts/build-windows-installer.ps1"),
      "utf8",
    );
    expect(buildScript).toContain("Ein Produktionsbuild darf das Release-Gate nicht überspringen.");
    expect(buildScript).toContain("release:gate");
    expect(buildScript).toContain("UpdateGuard.ps1");
    expect(buildScript).toContain("SignerCertificate.Issuer");
    expect(buildScript).toContain("SignerCertificate.Subject -eq");
    expect(buildScript).toContain("$installerSignature.Status -ne \"Valid\"");
  });
});
