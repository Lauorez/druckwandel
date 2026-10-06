import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "..");

describe("combined Windows installer", () => {
  it("bundles the printer into the Tauri NSIS installer", () => {
    const config = JSON.parse(
      readFileSync(resolve(root, "apps/desktop/src-tauri/tauri.conf.json"), "utf8"),
    );
    expect(config.bundle.windows.nsis.installMode).toBe("perMachine");
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
    expect(hooks).not.toContain("TrustPrinterCertificate.ps1");
    expect(hooks.match(/AssertSessionAccount\.ps1"'/g)).toHaveLength(2);
    expect(hooks.indexOf("AssertSessionAccount.ps1\"'")).toBeLessThan(hooks.indexOf("PrepareUpdate"));
    expect(hooks.lastIndexOf("AssertSessionAccount.ps1\"'")).toBeLessThan(hooks.indexOf("RemovePrinter.ps1\"'"));
    expect(hooks).toContain("UpdateGuard.ps1");
    expect(hooks).toContain("PrepareUpdate");
    expect(hooks).toContain("RecordInstalledVersion");
    expect(hooks).toContain("$UpdateMode = 1");
    expect(hooks).toContain("SetOutPath $INSTDIR");
    expect(hooks).toContain("Die vollständige Installation wird abgebrochen");
    expect(hooks).toContain("SetErrorLevel 1");
    expect(hooks).not.toContain("Die Anwendung wird trotzdem installiert");
  });

  it("replaces an install under the former name E-Rechnungs-Assistent", () => {
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

  it("installs only the expected signed printer package", () => {
    const installer = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/InstallPrinter.ps1"),
      "utf8",
    );
    expect(installer).toContain('$packageName = "ERechnung.VirtualPrinter.PoC"');
    expect(installer).toContain("Get-AuthenticodeSignature");
    expect(installer).toContain("Test-CertificateTrusted");
    expect(installer).toContain("Add-DevelopmentCertificateTrust");
    expect(installer).toContain('X509Store]::new("TrustedPeople", "LocalMachine")');
    expect(installer).toContain('$certificate.Subject -ne "CN=ERechnung Development"');
    expect(installer).toContain("$certificate.HasPrivateKey");
    expect(installer).not.toContain("-Verb RunAs");
    expect(installer).toContain("Add-AppxPackage");
    expect(installer).toContain("Wait-ForPrinter");
  });

  it("refuses to run under a different administrator account than the signed-in one", () => {
    const check = readFileSync(
      resolve(root, "apps/desktop/src-tauri/installer/windows/AssertSessionAccount.ps1"),
      "utf8",
    );
    expect(check).toContain("WindowsBuiltInRole]::Administrator");
    expect(check).toContain("Name = 'explorer.exe' AND SessionId = $sessionId");
    expect(check).toContain("GetOwnerSid");
    expect(check).toContain("$owner.Sid -ne $identity.User.Value");

    const buildScript = readFileSync(resolve(root, "scripts/build-windows-installer.ps1"), "utf8");
    expect(buildScript).toContain('"AssertSessionAccount.ps1"');
    expect(buildScript).not.toContain("TrustPrinterCertificate");
  });

  it("sets up a fresh Windows machine with one setup script", () => {
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

  it("prevents an unsigned production build", () => {
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
