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
    private readonly PrintJobStore store;
    private PrintJobRecord? job;
    private Deferral? workflowDeferral;

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
            PrintWorkflowJobUISession session = activationArgs.Session;
            session.VirtualPrinterUIDataAvailable += OnVirtualPrinterUiDataAvailable;
            session.Start();
        }
        else
        {
            _ = LoadJobAsync(store.GetLatest());
        }
    }

    private void OnVirtualPrinterUiDataAvailable(
        PrintWorkflowJobUISession sender,
        PrintWorkflowVirtualPrinterUIEventArgs args)
    {
        _ = sender;
        workflowDeferral = args.GetDeferral();
        PrintJobRecord? sessionJob = store.ResolveSession(args.Configuration.SessionId);

        DispatcherQueue.TryEnqueue(async () =>
        {
            await LoadJobAsync(sessionJob);

            if (job is null)
            {
                DocumentNameText.Text = args.Configuration.JobTitle;
                SourceApplicationText.Text = args.Configuration.SourceAppDisplayName;
                StatusText.Text = "Job metadata not found";
            }
        });
    }

    private async Task LoadJobAsync(PrintJobRecord? record)
    {
        job = record;
        if (record is null)
        {
            DocumentNameText.Text = "–";
            PdfPathText.Text = "–";
            PagesText.Text = "–";
            SourceApplicationText.Text = "–";
            JobIdText.Text = "–";
            StatusText.Text = "No print jobs found";
            return;
        }

        DocumentNameText.Text = record.DocumentName;
        PdfPathText.Text = record.PdfPath;
        SourceApplicationText.Text = record.SourceApplication ?? "–";
        JobIdText.Text = record.JobId.ToString("D");
        StatusText.Text = FormatStatus(record);
        OpenPdfButton.IsEnabled = File.Exists(record.PdfPath);
        OpenFolderButton.IsEnabled = Directory.Exists(Path.GetDirectoryName(record.PdfPath));

        PagesText.Text = await GetPageCountAsync(record.PdfPath);
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
            return "Unknown";
        }
    }

    private static string FormatStatus(PrintJobRecord record)
    {
        return record.Status switch
        {
            PrintJobStatus.PdfConversionSucceeded or
            PrintJobStatus.CompanionLaunchSucceeded or
            PrintJobStatus.JobCompleted => "Ready",
            PrintJobStatus.PdfConversionFailed or
            PrintJobStatus.CompanionLaunchFailed or
            PrintJobStatus.JobFailed => string.IsNullOrWhiteSpace(record.ErrorMessage)
                ? "Failed"
                : $"Failed – {record.ErrorMessage}",
            PrintJobStatus.JobCanceled => "Canceled",
            _ => record.Status.ToString()
        };
    }

    private async void OpenPdfButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (job is not null && File.Exists(job.PdfPath))
        {
            StorageFile file = await StorageFile.GetFileFromPathAsync(job.PdfPath);
            await Launcher.LaunchFileAsync(file);
        }
    }

    private async void OpenFolderButton_Click(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        string? folderPath = job is null ? null : Path.GetDirectoryName(job.PdfPath);
        if (folderPath is not null && Directory.Exists(folderPath))
        {
            StorageFolder folder = await StorageFolder.GetFolderFromPathAsync(folderPath);
            await Launcher.LaunchFolderAsync(folder);
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
        workflowDeferral?.Complete();
        workflowDeferral = null;
    }
}
