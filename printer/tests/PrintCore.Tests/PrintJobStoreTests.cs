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

    [Fact]
    public void ReviewHandoff_CopiesPdfAndWritesVersionedMetadata()
    {
        string sourceRoot = Path.Combine(root, "source");
        string reviewRoot = Path.Combine(root, "review");
        var store = new PrintJobStore(sourceRoot);
        PrintJobRecord job = store.Create("handoff-session", "E-Rechnung", "Rechnung 2026-0815", "Notepad.exe");
        File.WriteAllBytes(job.PdfPath, "%PDF-1.7\nbridge-test"u8.ToArray());
        job = store.Transition(job, PrintJobStatus.PdfConversionSucceeded, pages: 2);

        var handoffStore = new ReviewHandoffStore(reviewRoot);
        DateTimeOffset timestamp = DateTimeOffset.Parse("2026-08-30T10:00:00Z");
        ReviewHandoffResult result = handoffStore.Handoff(job, timestamp);

        Assert.Equal(File.ReadAllBytes(job.PdfPath), File.ReadAllBytes(result.PdfPath));
        Assert.Equal($"{job.JobId:D}.pdf", Path.GetFileName(result.PdfPath));
        Assert.Equal($"{job.JobId:D}.printjob.json", Path.GetFileName(result.MetadataPath));

        using JsonDocument document = JsonDocument.Parse(File.ReadAllText(result.MetadataPath));
        JsonElement metadata = document.RootElement;
        Assert.Equal(ReviewHandoffRecord.CurrentSchemaVersion, metadata.GetProperty("schemaVersion").GetInt32());
        Assert.Equal(job.JobId, metadata.GetProperty("jobId").GetGuid());
        Assert.Equal("Rechnung 2026-0815", metadata.GetProperty("documentName").GetString());
        Assert.Equal("Notepad.exe", metadata.GetProperty("sourceApplication").GetString());
        Assert.Equal(2, metadata.GetProperty("pages").GetInt32());
        Assert.Equal(timestamp, metadata.GetProperty("handedOffAt").GetDateTimeOffset());
    }

    [Fact]
    public void ReviewHandoff_RejectsAFileWithoutPdfSignature()
    {
        var store = new PrintJobStore(Path.Combine(root, "invalid-source"));
        PrintJobRecord job = store.Create("invalid-handoff", "E-Rechnung", "Kein PDF", null);
        File.WriteAllText(job.PdfPath, "not a PDF");

        var handoffStore = new ReviewHandoffStore(Path.Combine(root, "invalid-review"));

        InvalidDataException exception = Assert.Throws<InvalidDataException>(() => handoffStore.Handoff(job));
        Assert.Contains("PDF-Header", exception.Message);
        Assert.Empty(Directory.GetFiles(handoffStore.RootPath));
    }

    public void Dispose()
    {
        if (Directory.Exists(root))
        {
            Directory.Delete(root, true);
        }
    }
}
