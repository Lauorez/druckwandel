using ERechnung.PrintCore;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using Windows.Data.Pdf;
using Windows.Foundation;
using Windows.Graphics.Printing.Workflow;
using Windows.Storage;
using Windows.System;

namespace ERechnung.CompanionApp;

public sealed partial class CompanionPage : Page
{
    private const string ReviewInboxName = "E-Rechnung Druckeingang";
    private readonly PrintJobStore store;
    private PrintJobRecord? job;
    private Deferral? workflowDeferral;
    private PrintWorkflowJobUISession? workflowSession;
    private bool reviewHandoffStarted;

    public CompanionPage()
    {
        InitializeComponent();
        string root = Path.Combine(ApplicationData.Current.LocalFolder.Path, "ERechnung");
        store = new PrintJobStore(root);
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);

        if (e.Parameter is PrintWorkflowJobActivatedEventArgs activationArgs)
        {
            workflowSession = activationArgs.Session;
            workflowSession.VirtualPrinterUIDataAvailable += OnVirtualPrinterUiDataAvailable;
            try
            {
                workflowSession.Start();
            }
            catch (Exception exception)
            {
                workflowSession.VirtualPrinterUIDataAvailable -= OnVirtualPrinterUiDataAvailable;
                workflowSession = null;
                _ = exception;
                StatusText.Text = "Die Rechnung konnte nicht übernommen werden. Bitte drucken Sie sie erneut.";
            }
        }
        else if (e.Parameter is string activationWarning)
        {
            _ = LoadLatestWithWarningAsync(activationWarning);
        }
        else
        {
            _ = LoadJobAsync(store.GetLatest());
        }
    }

    private async Task LoadLatestWithWarningAsync(string warning)
    {
        await LoadJobAsync(store.GetLatest());
        _ = warning;
        StatusText.Text = "Die Rechnung konnte nicht automatisch geöffnet werden. Bitte klicken Sie auf „Im Assistenten öffnen“.";
    }

    private void OnVirtualPrinterUiDataAvailable(
        PrintWorkflowJobUISession sender,
        PrintWorkflowVirtualPrinterUIEventArgs args)
    {
        _ = sender;
        workflowDeferral?.Complete();
        workflowDeferral = args.GetDeferral();
        PrintJobRecord? sessionJob = store.ResolveSession(args.Configuration.SessionId);

        bool enqueued = DispatcherQueue.TryEnqueue(async () =>
        {
            await LoadJobAsync(sessionJob);

            if (job is null)
            {
                DocumentNameText.Text = args.Configuration.JobTitle;
                SourceApplicationText.Text = args.Configuration.SourceAppDisplayName;
                StatusText.Text = "Die Angaben zur gedruckten Rechnung fehlen. Bitte drucken Sie sie erneut.";
            }
        });

        if (!enqueued)
        {
            CompleteWorkflowDeferral();
        }
    }

    private async Task LoadJobAsync(PrintJobRecord? record)
    {
        job = record;
        if (record is null)
        {
            DocumentNameText.Text = "–";
            PagesText.Text = "–";
            SourceApplicationText.Text = "–";
            StatusText.Text = "Noch keine gedruckte Rechnung vorhanden";
            return;
        }

        DocumentNameText.Text = record.DocumentName;
        SourceApplicationText.Text = record.SourceApplication ?? "–";
        StatusText.Text = FormatStatus(record);
        OpenPdfButton.IsEnabled = File.Exists(record.PdfPath);
        OpenFolderButton.IsEnabled = Directory.Exists(Path.GetDirectoryName(record.PdfPath));
        OpenReviewButton.IsEnabled = File.Exists(record.PdfPath);

        PagesText.Text = await GetPageCountAsync(record.PdfPath);

        if (workflowSession is not null && !reviewHandoffStarted && File.Exists(record.PdfPath))
        {
            await HandoffAndOpenReviewAsync(closeOnSuccess: true);
        }
    }

    private static async Task<string> GetPageCountAsync(string pdfPath)
    {
        if (!File.Exists(pdfPath))
        {
            return "–";
        }

        try
        {
            StorageFile pdfFile = await StorageFile.GetFileFromPathAsync(pdfPath);
            PdfDocument document = await PdfDocument.LoadFromFileAsync(pdfFile);
            return document.PageCount.ToString();
        }
        catch
        {
            return "Unbekannt";
        }
    }

    private static string FormatStatus(PrintJobRecord record)
    {
        return record.Status switch
        {
            PrintJobStatus.PdfConversionSucceeded or
            PrintJobStatus.CompanionLaunchSucceeded or
            PrintJobStatus.JobCompleted => "Bereit",
            PrintJobStatus.PdfConversionFailed or
            PrintJobStatus.CompanionLaunchFailed or
            PrintJobStatus.JobFailed => "Fehlgeschlagen. Bitte drucken Sie die Rechnung erneut.",
            PrintJobStatus.JobCanceled => "Abgebrochen",
            _ => "Wird vorbereitet"
        };
    }

    private async void OpenPdfButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        try
        {
            if (job is not null && File.Exists(job.PdfPath))
            {
                StorageFile file = await StorageFile.GetFileFromPathAsync(job.PdfPath);
                bool launched = await Launcher.LaunchFileAsync(file);
                if (!launched)
                {
                    StatusText.Text = "Die Rechnung konnte nicht geöffnet werden.";
                }
            }
        }
        catch (Exception exception)
        {
            _ = exception;
            StatusText.Text = "Die Rechnung konnte nicht geöffnet werden.";
        }
    }

    private async void OpenReviewButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        await HandoffAndOpenReviewAsync(closeOnSuccess: workflowSession is not null);
    }

    private async Task HandoffAndOpenReviewAsync(bool closeOnSuccess)
    {
        if (job is null || reviewHandoffStarted)
        {
            return;
        }

        reviewHandoffStarted = true;
        OpenReviewButton.IsEnabled = false;
        try
        {
            string documentsPath = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
            if (string.IsNullOrWhiteSpace(documentsPath))
            {
                throw new DirectoryNotFoundException("Der Windows-Dokumenteordner konnte nicht ermittelt werden.");
            }

            string inboxPath = Path.Combine(documentsPath, ReviewInboxName);
            var handoffStore = new ReviewHandoffStore(inboxPath);
            ReviewHandoffResult handoff = await Task.Run(() => handoffStore.Handoff(job));

            var reviewUri = new Uri($"erechnung-review://print-job/{handoff.Metadata.JobId:D}");
            bool launched = await Launcher.LaunchUriAsync(reviewUri);
            if (!launched)
            {
                throw new InvalidOperationException(
                    "Druckwandel ist nicht installiert. Bitte installieren Sie die Anwendung und versuchen Sie es erneut.");
            }

            StatusText.Text = "In Druckwandel geöffnet";

            if (closeOnSuccess)
            {
                CompleteWorkflowDeferral();
                Application.Current.Exit();
            }
        }
        catch (Exception exception)
        {
            _ = exception;
            reviewHandoffStarted = false;
            OpenReviewButton.IsEnabled = job is not null && File.Exists(job.PdfPath);
            StatusText.Text = "Die Rechnung konnte nicht in Druckwandel geöffnet werden. Bitte prüfen Sie, ob Druckwandel installiert ist.";
        }
    }

    private async void OpenFolderButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        try
        {
            string? folderPath = job is null ? null : Path.GetDirectoryName(job.PdfPath);
            if (folderPath is not null && Directory.Exists(folderPath))
            {
                StorageFolder folder = await StorageFolder.GetFolderFromPathAsync(folderPath);
                bool launched = await Launcher.LaunchFolderAsync(folder);
                if (!launched)
                {
                    StatusText.Text = "Der Ordner konnte nicht geöffnet werden.";
                }
            }
        }
        catch (Exception exception)
        {
            _ = exception;
            StatusText.Text = "Der Ordner konnte nicht geöffnet werden.";
        }
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        CompleteWorkflowDeferral();
        Application.Current.Exit();
    }

    public void CompleteWorkflowDeferral()
    {
        if (workflowSession is not null)
        {
            workflowSession.VirtualPrinterUIDataAvailable -= OnVirtualPrinterUiDataAvailable;
            workflowSession = null;
        }

        workflowDeferral?.Complete();
        workflowDeferral = null;
    }
}
