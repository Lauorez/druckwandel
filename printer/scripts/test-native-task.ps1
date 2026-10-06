[CmdletBinding()]
param([string]$LibraryPath = "")
$ErrorActionPreference = "Stop"
if (-not $LibraryPath) {
    $workspace = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $LibraryPath = Join-Path $workspace "artifacts\printer-native\win-x64\ERechnung.VirtualPrinter.Tasks.dll"
}
$LibraryPath = (Resolve-Path -LiteralPath $LibraryPath).Path

# This process runs on Windows PowerShell/.NET Framework. A native activation
# must not load CoreCLR or resolve an installed .NET 10 runtime.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PrinterNativeActivationTest {
    [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr LoadLibrary(string path);
    [DllImport("kernel32", CharSet = CharSet.Ansi)]
    private static extern IntPtr GetProcAddress(IntPtr module, string name);
    [DllImport("combase", CharSet = CharSet.Unicode)]
    private static extern int WindowsCreateString(string value, int length, out IntPtr result);
    [DllImport("combase")]
    private static extern int WindowsDeleteString(IntPtr value);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    private delegate int GetFactory(IntPtr className, out IntPtr factory);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    private delegate int Activate(IntPtr factory, out IntPtr instance);
    public static void Run(string path) {
        IntPtr library = LoadLibrary(path);
        if (library == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        IntPtr export = GetProcAddress(library, "DllGetActivationFactory");
        if (export == IntPtr.Zero) throw new Exception("DllGetActivationFactory fehlt.");
        const string name = "ERechnung.VirtualPrinter.Tasks.VirtualPrinterBackgroundTask";
        IntPtr className, factory = IntPtr.Zero, instance = IntPtr.Zero;
        Marshal.ThrowExceptionForHR(WindowsCreateString(name, name.Length, out className));
        try {
            var get = (GetFactory)Marshal.GetDelegateForFunctionPointer(export, typeof(GetFactory));
            Marshal.ThrowExceptionForHR(get(className, out factory));
            IntPtr vtable = Marshal.ReadIntPtr(factory);
            var activate = (Activate)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(vtable, 6 * IntPtr.Size), typeof(Activate));
            Marshal.ThrowExceptionForHR(activate(factory, out instance));
            if (instance == IntPtr.Zero) throw new Exception("Leere Background-Task-Instanz.");
            Guid taskInterface = new Guid("7d13d534-fd12-43ce-8c22-ea1ff13c06df");
            IntPtr backgroundTask;
            Marshal.ThrowExceptionForHR(Marshal.QueryInterface(instance, ref taskInterface, out backgroundTask));
            Marshal.Release(backgroundTask);
        } finally {
            if (instance != IntPtr.Zero) Marshal.Release(instance);
            if (factory != IntPtr.Zero) Marshal.Release(factory);
            WindowsDeleteString(className);
        }
    }
}
'@
[PrinterNativeActivationTest]::Run($LibraryPath)
$coreclr = (Get-Process -Id $PID).Modules | Where-Object ModuleName -EQ "coreclr.dll"
if ($coreclr) { throw "Der Background-Task hat unerwartet CoreCLR geladen." }
Write-Output "PASS: nativer Background-Task aktiviert, kein CoreCLR geladen."
