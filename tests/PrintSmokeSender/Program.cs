using System.IO;
using System.Printing;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Xps;
using System.Windows.Xps.Packaging;

namespace ERechnung.PrintSmokeSender;

internal static class Program
{
    private const string PrinterName = "E-Rechnung";
    private const string DocumentName = "E-Rechnung automated smoke test";
    private const double A4Width = 793.7008;
    private const double A4Height = 1122.5197;

    [STAThread]
    private static void Main()
    {
        string xpsPath = Path.Combine(Path.GetTempPath(), $"erechnung-smoke-{Guid.NewGuid():N}.xps");
        try
        {
            CreateTestDocument(xpsPath);

            using var server = new LocalPrintServer();
            using PrintQueue queue = server.GetPrintQueue(PrinterName);
            PrintTicket ticket = queue.DefaultPrintTicket;
            ticket.PageMediaSize = new PageMediaSize(PageMediaSizeName.ISOA4);
            ticket.PageOrientation = PageOrientation.Portrait;

            PrintSystemJobInfo job = queue.AddJob(DocumentName, xpsPath, false, ticket);
            Console.WriteLine(
                $"Submitted '{DocumentName}' to '{PrinterName}' as spooler job {job.JobIdentifier}.");
        }
        finally
        {
            try
            {
                File.Delete(xpsPath);
            }
            catch (IOException)
            {
                // The smoke test must not hide its actual result behind temporary-file cleanup.
            }
            catch (UnauthorizedAccessException)
            {
                // The smoke test must not hide its actual result behind temporary-file cleanup.
            }
        }
    }

    private static void CreateTestDocument(string xpsPath)
    {
        FixedDocument document = new();
        document.DocumentPaginator.PageSize = new Size(A4Width, A4Height);

        FixedPage page = new()
        {
            Width = A4Width,
            Height = A4Height,
            Background = Brushes.White
        };

        AddText(page, "E-Rechnung Virtual Printer", 80, 80, 24, FontWeights.Bold, Brushes.DarkBlue);
        AddText(
            page,
            "Automated local print-pipeline smoke test.\nNo invoice processing is performed.",
            80,
            140,
            14,
            FontWeights.Normal,
            Brushes.Black);

        Border outline = new()
        {
            Width = 500,
            Height = 180,
            BorderBrush = Brushes.DarkBlue,
            BorderThickness = new Thickness(2)
        };
        FixedPage.SetLeft(outline, 80);
        FixedPage.SetTop(outline, 220);
        page.Children.Add(outline);

        AddText(
            page,
            DateTimeOffset.UtcNow.ToString("O"),
            100,
            250,
            14,
            FontWeights.Normal,
            Brushes.Black);

        PageContent pageContent = new();
        ((IAddChild)pageContent).AddChild(page);
        document.Pages.Add(pageContent);

        using var xpsDocument = new XpsDocument(xpsPath, FileAccess.ReadWrite);
        XpsDocumentWriter writer = XpsDocument.CreateXpsDocumentWriter(xpsDocument);
        writer.Write(document);
    }

    private static void AddText(
        FixedPage page,
        string text,
        double left,
        double top,
        double fontSize,
        FontWeight fontWeight,
        Brush foreground)
    {
        TextBlock textBlock = new()
        {
            Text = text,
            FontFamily = new FontFamily("Segoe UI"),
            FontSize = fontSize,
            FontWeight = fontWeight,
            Foreground = foreground
        };
        FixedPage.SetLeft(textBlock, left);
        FixedPage.SetTop(textBlock, top);
        page.Children.Add(textBlock);
    }
}
