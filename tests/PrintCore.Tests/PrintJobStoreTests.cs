using ERechnung.PrintCore;
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

    public void Dispose()
    {
        if (Directory.Exists(root))
        {
            Directory.Delete(root, true);
        }
    }
}
