using System.Text.Json;
using System.Text.Json.Serialization;

namespace ERechnung.PrintCore;

public sealed class PrintJobStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) }
    };

    private static readonly JsonSerializerOptions EventJsonOptions = new(JsonOptions)
    {
        WriteIndented = false
    };

    public PrintJobStore(string rootPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(rootPath);
        RootPath = Path.GetFullPath(rootPath);
        JobsPath = Path.Combine(RootPath, "PrintJobs");
        SessionsPath = Path.Combine(RootPath, "Sessions");
        LogsPath = Path.Combine(RootPath, "Logs");

        Directory.CreateDirectory(JobsPath);
        Directory.CreateDirectory(SessionsPath);
        Directory.CreateDirectory(LogsPath);
    }

    public string RootPath { get; }

    public string JobsPath { get; }

    public string SessionsPath { get; }

    public string LogsPath { get; }

    public PrintJobRecord Create(
        string sessionId,
        string printerName,
        string documentName,
        string? sourceApplication,
        DateTimeOffset? now = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sessionId);

        Guid jobId = Guid.NewGuid();
        DateTimeOffset timestamp = now ?? DateTimeOffset.UtcNow;
        var job = new PrintJobRecord
        {
            JobId = jobId,
            SessionId = sessionId,
            CreatedAt = timestamp,
            UpdatedAt = timestamp,
            PdfPath = Path.Combine(JobsPath, $"{jobId:D}.pdf"),
            PrinterName = string.IsNullOrWhiteSpace(printerName) ? "E-Rechnung" : printerName,
            DocumentName = string.IsNullOrWhiteSpace(documentName) ? "Unbenanntes Dokument" : documentName,
            SourceApplication = string.IsNullOrWhiteSpace(sourceApplication) ? null : sourceApplication,
            Status = PrintJobStatus.PrintJobReceived
        };

        Save(job);
        WriteSessionIndex(job);
        AppendEvent(job, PrintJobStatus.PrintJobReceived);
        return job;
    }

    public PrintJobRecord Transition(
        PrintJobRecord job,
        PrintJobStatus status,
        string? message = null,
        int? pages = null)
    {
        ArgumentNullException.ThrowIfNull(job);

        PrintJobRecord updated = job with
        {
            Status = status,
            UpdatedAt = DateTimeOffset.UtcNow,
            Pages = pages ?? job.Pages,
            ErrorMessage = status is PrintJobStatus.JobFailed or PrintJobStatus.PdfConversionFailed
                or PrintJobStatus.CompanionLaunchFailed
                ? message
                : job.ErrorMessage
        };

        Save(updated);
        AppendEvent(updated, status, message);
        return updated;
    }

    public PrintJobRecord? Get(Guid jobId)
    {
        string path = GetJobJsonPath(jobId);
        return File.Exists(path) ? Read(path) : null;
    }

    public PrintJobRecord? ResolveSession(string sessionId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sessionId);
        string indexPath = Path.Combine(SessionsPath, $"{SessionKey.From(sessionId)}.txt");
        if (!File.Exists(indexPath))
        {
            return null;
        }

        return Guid.TryParse(File.ReadAllText(indexPath).Trim(), out Guid jobId)
            ? Get(jobId)
            : null;
    }

    public PrintJobRecord? GetLatest()
    {
        return Directory.EnumerateFiles(JobsPath, "*.json", SearchOption.TopDirectoryOnly)
            .Select(Read)
            .Where(static job => job is not null)
            .OrderByDescending(static job => job!.CreatedAt)
            .FirstOrDefault();
    }

    public string GetTemporaryPdfPath(Guid jobId) => Path.Combine(JobsPath, $"{jobId:D}.pdf.tmp");

    private static PrintJobRecord? Read(string path)
    {
        try
        {
            return JsonSerializer.Deserialize<PrintJobRecord>(File.ReadAllText(path), JsonOptions);
        }
        catch (JsonException)
        {
            return null;
        }
        catch (IOException)
        {
            return null;
        }
    }

    private void Save(PrintJobRecord job)
    {
        AtomicWrite(GetJobJsonPath(job.JobId), JsonSerializer.Serialize(job, JsonOptions));
    }

    private void WriteSessionIndex(PrintJobRecord job)
    {
        string path = Path.Combine(SessionsPath, $"{SessionKey.From(job.SessionId)}.txt");
        AtomicWrite(path, job.JobId.ToString("D"));
    }

    private void AppendEvent(PrintJobRecord job, PrintJobStatus status, string? message = null)
    {
        var entry = new PrintJobEvent(DateTimeOffset.UtcNow, job.JobId, status, message);
        string path = Path.Combine(LogsPath, $"{job.JobId:D}.jsonl");
        File.AppendAllText(path, JsonSerializer.Serialize(entry, EventJsonOptions) + Environment.NewLine);
    }

    private string GetJobJsonPath(Guid jobId) => Path.Combine(JobsPath, $"{jobId:D}.json");

    private static void AtomicWrite(string path, string content)
    {
        string temporaryPath = path + $".{Guid.NewGuid():N}.tmp";
        try
        {
            File.WriteAllText(temporaryPath, content);
            File.Move(temporaryPath, path, true);
        }
        finally
        {
            TryDelete(temporaryPath);
        }
    }

    private static void TryDelete(string path)
    {
        try
        {
            File.Delete(path);
        }
        catch (IOException)
        {
            // Best-effort cleanup must not replace the original write/move result.
        }
        catch (UnauthorizedAccessException)
        {
            // Best-effort cleanup must not replace the original write/move result.
        }
    }
}
