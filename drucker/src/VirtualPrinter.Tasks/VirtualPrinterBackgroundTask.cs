using ERechnung.PrintCore;
using Windows.ApplicationModel.Background;
using Windows.Devices.Printers;
using Windows.Data.Pdf;
using Windows.Graphics.Printing.Workflow;
using Windows.Storage;
using Windows.Storage.Streams;

namespace ERechnung.VirtualPrinter.Tasks;

public sealed class VirtualPrinterBackgroundTask : IBackgroundTask
{
    private BackgroundTaskDeferral? taskDeferral;
    private IppPrintDevice? printer;

    public void Run(IBackgroundTaskInstance taskInstance)
    {
        taskDeferral = taskInstance.GetDeferral();

        if (taskInstance.TriggerDetails is not PrintWorkflowVirtualPrinterTriggerDetails triggerDetails)
        {
            taskDeferral.Complete();
            return;
        }

        PrintWorkflowVirtualPrinterSession session = triggerDetails.VirtualPrinterSession;
        printer = session.Printer;
        session.VirtualPrinterDataAvailable += OnVirtualPrinterDataAvailable;
        try
        {
            session.Start();
        }
        catch
        {
            taskDeferral.Complete();
            throw;
        }
    }

    private async void OnVirtualPrinterDataAvailable(
        PrintWorkflowVirtualPrinterSession sender,
        PrintWorkflowVirtualPrinterDataAvailableEventArgs args)
    {
        _ = sender;
        PrintWorkflowSubmittedStatus submittedStatus = PrintWorkflowSubmittedStatus.Failed;
        PrintJobStore? store = null;
        PrintJobRecord? job = null;
        string? temporaryPdfPath = null;

        try
        {
            string appDataPath = Path.Combine(ApplicationData.Current.LocalFolder.Path, "ERechnung");
            store = new PrintJobStore(appDataPath);

            PrintWorkflowConfiguration configuration = args.Configuration;
            job = store.Create(
                configuration.SessionId,
                printer?.PrinterName ?? "E-Rechnung",
                configuration.JobTitle,
                configuration.SourceAppDisplayName);

            job = store.Transition(job, PrintJobStatus.PdfConversionStarted);
            temporaryPdfPath = store.GetTemporaryPdfPath(job.JobId);

            await ConvertToPdfAsync(args, store, job);
            int pages = await GetPdfPageCountAsync(job.PdfPath);
            job = store.Transition(job, PrintJobStatus.PdfConversionSucceeded, pages: pages);

            if (!args.UILauncher.IsUILaunchEnabled())
            {
                job = store.Transition(
                    job,
                    PrintJobStatus.CompanionLaunchFailed,
                    "Windows hat die PrintSupportJobUI-Aktivierung für diesen Job nicht freigegeben.");
                throw new InvalidOperationException(job.ErrorMessage ?? "Companion-App konnte nicht gestartet werden.");
            }

            PrintWorkflowUICompletionStatus uiStatus =
                await args.UILauncher.LaunchAndCompleteUIAsync();

            if (uiStatus == PrintWorkflowUICompletionStatus.UserCanceled)
            {
                store.Transition(job, PrintJobStatus.JobCanceled, "Companion-App wurde abgebrochen.");
                submittedStatus = PrintWorkflowSubmittedStatus.Canceled;
                return;
            }

            if (uiStatus != PrintWorkflowUICompletionStatus.Completed)
            {
                job = store.Transition(
                    job,
                    PrintJobStatus.CompanionLaunchFailed,
                    $"Companion-App lieferte Status {uiStatus}.");
                throw new InvalidOperationException(job.ErrorMessage ?? "Companion-App konnte nicht gestartet werden.");
            }

            job = store.Transition(job, PrintJobStatus.CompanionLaunchSucceeded);
            store.Transition(job, PrintJobStatus.JobCompleted);
            submittedStatus = PrintWorkflowSubmittedStatus.Succeeded;
        }
        catch (Exception exception)
        {
            if (store is not null && job is not null)
            {
                PrintJobStatus failureStatus = job.Status == PrintJobStatus.PdfConversionStarted
                    ? PrintJobStatus.PdfConversionFailed
                    : PrintJobStatus.JobFailed;
                job = store.Transition(job, failureStatus, exception.Message);

                if (failureStatus != PrintJobStatus.JobFailed)
                {
                    store.Transition(job, PrintJobStatus.JobFailed, exception.Message);
                }
            }
        }
        finally
        {
            try
            {
                if (temporaryPdfPath is not null && File.Exists(temporaryPdfPath))
                {
                    File.Delete(temporaryPdfPath);
                }
            }
            catch (IOException)
            {
                // Cleanup must never prevent the print workflow from being completed.
            }
            catch (UnauthorizedAccessException)
            {
                // Cleanup must never prevent the print workflow from being completed.
            }

            try
            {
                args.CompleteJob(submittedStatus);
            }
            finally
            {
                taskDeferral?.Complete();
            }
        }
    }

    private static async Task ConvertToPdfAsync(
        PrintWorkflowVirtualPrinterDataAvailableEventArgs args,
        PrintJobStore store,
        PrintJobRecord job)
    {
        StorageFolder jobsFolder = await StorageFolder.GetFolderFromPathAsync(store.JobsPath);
        StorageFile temporaryFile = await jobsFolder.CreateFileAsync(
            $"{job.JobId:D}.pdf.tmp",
            CreationCollisionOption.FailIfExists);

        using (IRandomAccessStream randomAccessStream = await temporaryFile.OpenAsync(FileAccessMode.ReadWrite))
        using (IOutputStream outputStream = randomAccessStream.GetOutputStreamAt(0))
        using (IInputStream inputStream = args.SourceContent.GetInputStream())
        {
            string contentType = args.SourceContent.ContentType;
            if (string.Equals(contentType, "application/oxps", StringComparison.OrdinalIgnoreCase))
            {
                PrintWorkflowPdlConverter converter =
                    args.GetPdlConverter(PrintWorkflowPdlConversionType.XpsToPdf);
                await converter.ConvertPdlAsync(args.GetJobPrintTicket(), inputStream, outputStream);
            }
            else if (string.Equals(contentType, "application/pdf", StringComparison.OrdinalIgnoreCase))
            {
                await RandomAccessStream.CopyAsync(inputStream, outputStream);
            }
            else
            {
                throw new InvalidDataException($"Nicht unterstütztes Eingabeformat: {contentType}");
            }

            await outputStream.FlushAsync();
        }

        await temporaryFile.RenameAsync($"{job.JobId:D}.pdf", NameCollisionOption.FailIfExists);
    }

    private static async Task<int> GetPdfPageCountAsync(string pdfPath)
    {
        StorageFile pdfFile = await StorageFile.GetFileFromPathAsync(pdfPath);
        PdfDocument document = await PdfDocument.LoadFromFileAsync(pdfFile);
        return checked((int)document.PageCount);
    }

}
