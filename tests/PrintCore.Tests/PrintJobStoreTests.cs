using ERechnung.PrintCore;
using System.Text.Json;
using Xunit;

namespace ERechnung.PrintCore.Tests;

public sealed class PrintJobStoreTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), $"erechnung-tests-{Guid.NewGuid():N}");

    [Fact]
    public void Create_PersistsJobAndSessionIndex()
    {
        var store = new PrintJobStore(root);

        PrintJobRecord created = store.Create("session/one", "E-Rechnung", "Testrechnung", "Notepad");
        PrintJobRecord? resolved = store.ResolveSession("session/one");

        Assert.NotNull(resolved);
        Assert.Equal(created.JobId, resolved.JobId);
        Assert.Equal(PrintJobStatus.PrintJobReceived, resolved.Status);
        Assert.EndsWith($"{created.JobId:D}.pdf", resolved.PdfPath);
    }

    [Fact]
    public void Transition_AtomicallyUpdatesStatusAndLog()
    {
        var store = new PrintJobStore(root);
        PrintJobRecord job = store.Create("session-two", "E-Rechnung", "Mehrseitig", null);

        job = store.Transition(job, PrintJobStatus.PdfConversionStarted);
        job = store.Transition(job, PrintJobStatus.PdfConversionSucceeded, pages: 3);

        PrintJobRecord persisted = Assert.IsType<PrintJobRecord>(store.Get(job.JobId));
        Assert.Equal(PrintJobStatus.PdfConversionSucceeded, persisted.Status);
        Assert.Equal(3, persisted.Pages);
        Assert.Equal(3, File.ReadLines(Path.Combine(store.LogsPath, $"{job.JobId:D}.jsonl")).Count());
    }

    [Fact]
    public async Task ConcurrentJobsRemainIndependent()
    {
        var store = new PrintJobStore(root);

        PrintJobRecord[] jobs = await Task.WhenAll(
            Enumerable.Range(0, 8).Select(i => Task.Run(() =>
                store.Create($"session-{i}", "E-Rechnung", "Gleicher Dokumentname", null))));

        Assert.Equal(8, jobs.Select(static job => job.JobId).Distinct().Count());
        Assert.Equal(8, Directory.GetFiles(store.JobsPath, "*.json").Length);
        Assert.All(jobs, job => Assert.Equal(job.JobId, store.ResolveSession(job.SessionId)?.JobId));
    }

    [Fact]
    public void SessionKey_DoesNotExposeOrInterpretSessionAsPath()
    {
        string key = SessionKey.From("../unsafe/session\\name");

        Assert.Equal(64, key.Length);
        Assert.DoesNotContain("/", key);
        Assert.DoesNotContain("\\", key);
        Assert.DoesNotContain("..", key);
    }

    [Fact]
    public void JsonContract_UsesExpectedNamesAndExplicitNull()
    {
        var store = new PrintJobStore(root);
        PrintJobRecord job = store.Create("json-session", "E-Rechnung", "Test 123", null);
        string jsonPath = Path.Combine(store.JobsPath, $"{job.JobId:D}.json");

        using JsonDocument document = JsonDocument.Parse(File.ReadAllText(jsonPath));
        JsonElement rootElement = document.RootElement;

        Assert.Equal(job.JobId, rootElement.GetProperty("jobId").GetGuid());
        Assert.Equal("E-Rechnung", rootElement.GetProperty("printerName").GetString());
        Assert.Equal("Test 123", rootElement.GetProperty("documentName").GetString());
        Assert.Equal(JsonValueKind.Null, rootElement.GetProperty("sourceApplication").ValueKind);
        Assert.Equal("printJobReceived", rootElement.GetProperty("status").GetString());
    }

    [Fact]
    public void GetLatest_IgnoresCorruptMetadataFile()
    {
        var store = new PrintJobStore(root);
        PrintJobRecord job = store.Create("valid-session", "E-Rechnung", "Valid", null);
        File.WriteAllText(Path.Combine(store.JobsPath, "corrupt.json"), "{not json");

        PrintJobRecord latest = Assert.IsType<PrintJobRecord>(store.GetLatest());

        Assert.Equal(job.JobId, latest.JobId);
    }

    [Fact]
    public void Create_NormalizesMissingDisplayMetadata()
    {
        var store = new PrintJobStore(root);

        PrintJobRecord job = store.Create("fallback-session", "  ", "", "\t");

        Assert.Equal("E-Rechnung", job.PrinterName);
        Assert.Equal("Unbenanntes Dokument", job.DocumentName);
        Assert.Null(job.SourceApplication);
    }

    [Fact]
    public void FailureTransition_PersistsMessageInMetadataAndEventLog()
    {
        var store = new PrintJobStore(root);
        PrintJobRecord job = store.Create("failure-session", "E-Rechnung", "Defekt", null);

        job = store.Transition(job, PrintJobStatus.PdfConversionFailed, "conversion exploded");

        PrintJobRecord persisted = Assert.IsType<PrintJobRecord>(store.Get(job.JobId));
        Assert.Equal(PrintJobStatus.PdfConversionFailed, persisted.Status);
        Assert.Equal("conversion exploded", persisted.ErrorMessage);
        Assert.Contains(
            "conversion exploded",
            File.ReadAllText(Path.Combine(store.LogsPath, $"{job.JobId:D}.jsonl")));
    }

    public void Dispose()
    {
        if (Directory.Exists(root))
        {
            Directory.Delete(root, true);
        }
    }
}
