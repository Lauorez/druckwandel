namespace ERechnung.PrintCore;

public enum PrintJobStatus
{
    PrintJobReceived,
    PdfConversionStarted,
    PdfConversionSucceeded,
    PdfConversionFailed,
    CompanionLaunchSucceeded,
    CompanionLaunchFailed,
    JobCompleted,
    JobFailed,
    JobCanceled
}
