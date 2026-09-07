namespace ERechnung.PrintCore;

public sealed record PrintJobRecord
{
    public required Guid JobId { get; init; }

    public required string SessionId { get; init; }

    public required DateTimeOffset CreatedAt { get; init; }

    public required DateTimeOffset UpdatedAt { get; init; }

    public required string PdfPath { get; init; }

    public required string PrinterName { get; init; }

    public required string DocumentName { get; init; }

    public string? SourceApplication { get; init; }

    public int? Pages { get; init; }

    public required PrintJobStatus Status { get; init; }

    public string? ErrorMessage { get; init; }
}
