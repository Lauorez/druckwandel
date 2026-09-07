namespace ERechnung.PrintCore;

public sealed record PrintJobEvent(
    DateTimeOffset Timestamp,
    Guid JobId,
    PrintJobStatus Status,
    string? Message = null);
