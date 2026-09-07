namespace ERechnung.PrintCore;

public sealed record ReviewHandoffRecord
{
    public const int CurrentSchemaVersion = 1;

    public required int SchemaVersion { get; init; }

    public required Guid JobId { get; init; }

    public required DateTimeOffset HandedOffAt { get; init; }

    public required string PdfFileName { get; init; }

    public required string DocumentName { get; init; }

    public string? SourceApplication { get; init; }

    public required string PrinterName { get; init; }

    public int? Pages { get; init; }
}

public sealed record ReviewHandoffResult(
    ReviewHandoffRecord Metadata,
    string PdfPath,
    string MetadataPath);
