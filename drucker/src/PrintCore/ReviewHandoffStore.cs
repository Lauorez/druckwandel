using System.Text;
using System.Text.Json;

namespace ERechnung.PrintCore;

public sealed class ReviewHandoffStore
{
    private static readonly byte[] PdfSignature = "%PDF-"u8.ToArray();

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true
    };

    public ReviewHandoffStore(string rootPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(rootPath);
        RootPath = Path.GetFullPath(rootPath);
        Directory.CreateDirectory(RootPath);
    }

    public string RootPath { get; }

    public ReviewHandoffResult Handoff(PrintJobRecord job, DateTimeOffset? now = null)
    {
        ArgumentNullException.ThrowIfNull(job);

        string sourcePdfPath = Path.GetFullPath(job.PdfPath);
        EnsurePdf(sourcePdfPath);

        string jobKey = job.JobId.ToString("D");
        string pdfFileName = $"{jobKey}.pdf";
        string metadataFileName = $"{jobKey}.printjob.json";
        string targetPdfPath = Path.Combine(RootPath, pdfFileName);
        string metadataPath = Path.Combine(RootPath, metadataFileName);
        string temporaryPdfPath = Path.Combine(RootPath, $".{jobKey}.{Guid.NewGuid():N}.pdf.tmp");

        var metadata = new ReviewHandoffRecord
        {
            SchemaVersion = ReviewHandoffRecord.CurrentSchemaVersion,
            JobId = job.JobId,
            HandedOffAt = now ?? DateTimeOffset.UtcNow,
            PdfFileName = pdfFileName,
            DocumentName = job.DocumentName,
            SourceApplication = job.SourceApplication,
            PrinterName = job.PrinterName,
            Pages = job.Pages
        };

        try
        {
            using (FileStream source = new(sourcePdfPath, FileMode.Open, FileAccess.Read, FileShare.Read))
            using (FileStream target = new(
                temporaryPdfPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 81920,
                FileOptions.SequentialScan))
            {
                source.CopyTo(target);
                target.Flush(flushToDisk: true);
            }

            File.Move(temporaryPdfPath, targetPdfPath, overwrite: true);
            AtomicWrite(metadataPath, JsonSerializer.Serialize(metadata, JsonOptions));
        }
        finally
        {
            TryDelete(temporaryPdfPath);
        }

        return new ReviewHandoffResult(metadata, targetPdfPath, metadataPath);
    }

    private static void EnsurePdf(string path)
    {
        if (!File.Exists(path))
        {
            throw new FileNotFoundException("Das PDF des Druckjobs wurde nicht gefunden.", path);
        }

        Span<byte> header = stackalloc byte[5];
        using FileStream stream = new(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Read(header) != PdfSignature.Length || !header.SequenceEqual(PdfSignature))
        {
            throw new InvalidDataException("Der Druckjob besitzt keinen gültigen PDF-Header.");
        }
    }

    private static void AtomicWrite(string path, string content)
    {
        string temporaryPath = path + $".{Guid.NewGuid():N}.tmp";
        try
        {
            using (FileStream stream = new(
                temporaryPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 4096,
                FileOptions.SequentialScan))
            {
                byte[] bytes = Encoding.UTF8.GetBytes(content);
                stream.Write(bytes);
                stream.Flush(flushToDisk: true);
            }

            File.Move(temporaryPath, path, overwrite: true);
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
            // Best-effort cleanup must not replace the original handoff error.
        }
        catch (UnauthorizedAccessException)
        {
            // Best-effort cleanup must not replace the original handoff error.
        }
    }
}
