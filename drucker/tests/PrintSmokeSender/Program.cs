using System.IO;
using System.Printing;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Xps;
using System.Windows.Xps.Packaging;
using Rectangle = System.Windows.Shapes.Rectangle;

namespace ERechnung.PrintSmokeSender;

internal static class Program
{
    private const string PrinterName = "E-Rechnung";
    private const double A4Width = 793.7008;
    private const double A4Height = 1122.5197;

    [STAThread]
    private static int Main(string[] args)
    {
        if (!TryParseScenario(args, out SmokeScenario scenario))
        {
            Console.Error.WriteLine("Usage: PrintSmokeSender [--scenario basic|multipage|landscape|visual]");
            return 2;
        }

        ScenarioDefinition definition = GetDefinition(scenario);
        string xpsPath = Path.Combine(Path.GetTempPath(), $"erechnung-smoke-{Guid.NewGuid():N}.xps");
        try
        {
            CreateTestDocument(xpsPath, definition);
            using var server = new LocalPrintServer();
            using PrintQueue queue = server.GetPrintQueue(PrinterName);
            PrintTicket ticket = queue.DefaultPrintTicket;
            ticket.PageMediaSize = new PageMediaSize(PageMediaSizeName.ISOA4);
            ticket.PageOrientation = definition.Landscape ? PageOrientation.Landscape : PageOrientation.Portrait;

            PrintSystemJobInfo job = queue.AddJob(definition.DocumentName, xpsPath, false, ticket);
            Console.WriteLine(
                $"Submitted scenario '{definition.Name}' as '{definition.DocumentName}' " +
                $"with {definition.PageCount} page(s), spooler job {job.JobIdentifier}.");
            return 0;
        }
        finally
        {
            TryDelete(xpsPath);
        }
    }

    private static bool TryParseScenario(string[] args, out SmokeScenario scenario)
    {
        scenario = SmokeScenario.Basic;
        return args.Length == 0 ||
            (args.Length == 2 &&
             string.Equals(args[0], "--scenario", StringComparison.OrdinalIgnoreCase) &&
             Enum.TryParse(args[1], true, out scenario) &&
             Enum.IsDefined(scenario));
    }

    private static ScenarioDefinition GetDefinition(SmokeScenario scenario) => scenario switch
    {
        SmokeScenario.Basic => new("basic", "E-Rechnung automated smoke test", 1, false, false),
        SmokeScenario.Multipage => new("multipage", "E-Rechnung smoke multipage", 3, false, false),
        SmokeScenario.Landscape => new("landscape", "E-Rechnung smoke landscape", 1, true, false),
        SmokeScenario.Visual => new("visual", "E-Rechnung smoke visual content", 1, false, true),
        _ => throw new ArgumentOutOfRangeException(nameof(scenario))
    };

    private static void CreateTestDocument(string xpsPath, ScenarioDefinition definition)
    {
        double width = definition.Landscape ? A4Height : A4Width;
        double height = definition.Landscape ? A4Width : A4Height;
        FixedDocument document = new();
        document.DocumentPaginator.PageSize = new Size(width, height);

        for (int pageNumber = 1; pageNumber <= definition.PageCount; pageNumber++)
        {
            FixedPage page = CreatePage(width, height, definition, pageNumber);
            PageContent pageContent = new();
            ((IAddChild)pageContent).AddChild(page);
            document.Pages.Add(pageContent);
        }

        using var xpsDocument = new XpsDocument(xpsPath, FileAccess.ReadWrite);
        XpsDocumentWriter writer = XpsDocument.CreateXpsDocumentWriter(xpsDocument);
        writer.Write(document);
    }

    private static FixedPage CreatePage(double width, double height, ScenarioDefinition definition, int pageNumber)
    {
        FixedPage page = new() { Width = width, Height = height, Background = Brushes.White };
        AddText(page, "E-Rechnung Virtual Printer", 64, 56, 24, FontWeights.Bold, Brushes.DarkBlue);
        AddText(page, $"Scenario: {definition.Name} · Page {pageNumber}/{definition.PageCount}", 64, 105, 14, FontWeights.SemiBold, Brushes.Black);
        AddText(page, DateTimeOffset.UtcNow.ToString("O"), 64, 136, 11, FontWeights.Normal, Brushes.DimGray);

        Border outline = new()
        {
            Width = Math.Min(560, width - 128),
            Height = 150,
            BorderBrush = Brushes.DarkBlue,
            BorderThickness = new Thickness(2),
            CornerRadius = new CornerRadius(8)
        };
        Place(page, outline, 64, 185);
        AddText(page, "Automated local print-pipeline acceptance test.\nNo invoice processing is performed.", 88, 220, 14, FontWeights.Normal, Brushes.Black);

        if (definition.VisualContent)
        {
            AddVisualContent(page);
        }

        AddText(page, $"END-OF-PAGE-{pageNumber}", 64, height - 70, 10, FontWeights.Bold, Brushes.DarkRed);
        return page;
    }

    private static void AddVisualContent(FixedPage page)
    {
        Rectangle colorBand = new()
        {
            Width = 560,
            Height = 70,
            Fill = new LinearGradientBrush(Colors.OrangeRed, Colors.DodgerBlue, 0),
            Stroke = Brushes.Black,
            StrokeThickness = 1
        };
        Place(page, colorBand, 64, 375);

        Grid table = new() { Width = 560, Height = 180 };
        for (int column = 0; column < 3; column++) table.ColumnDefinitions.Add(new ColumnDefinition());
        for (int row = 0; row < 4; row++) table.RowDefinitions.Add(new RowDefinition());
        string[,] values =
        {
            { "Position", "Quantity", "Amount" },
            { "Service A", "2", "120.00" },
            { "Service B", "1", "80.00" },
            { "Total", "", "200.00" }
        };
        for (int row = 0; row < 4; row++)
        {
            for (int column = 0; column < 3; column++)
            {
                Border cell = new()
                {
                    BorderBrush = Brushes.SlateGray,
                    BorderThickness = new Thickness(0.5),
                    Background = row == 0 ? Brushes.LightSteelBlue : Brushes.White,
                    Child = new TextBlock
                    {
                        Text = values[row, column],
                        Margin = new Thickness(8),
                        FontWeight = row is 0 or 3 ? FontWeights.Bold : FontWeights.Normal
                    }
                };
                Grid.SetRow(cell, row);
                Grid.SetColumn(cell, column);
                table.Children.Add(cell);
            }
        }
        Place(page, table, 64, 485);

        DrawingGroup drawing = new();
        drawing.Children.Add(new GeometryDrawing(Brushes.Gold, new Pen(Brushes.DarkOrange, 4), new EllipseGeometry(new Point(70, 70), 58, 58)));
        drawing.Children.Add(new GeometryDrawing(Brushes.ForestGreen, new Pen(Brushes.DarkGreen, 3), new RectangleGeometry(new Rect(135, 20, 120, 100), 12, 12)));
        Image image = new() { Width = 280, Height = 140, Source = new DrawingImage(drawing), Stretch = Stretch.Uniform };
        Place(page, image, 64, 705);
        AddText(page, "Vector image + color + table + Segoe UI text", 360, 755, 14, FontWeights.SemiBold, Brushes.Purple);
    }

    private static void AddText(FixedPage page, string text, double left, double top, double fontSize, FontWeight fontWeight, Brush foreground)
    {
        TextBlock textBlock = new()
        {
            Text = text,
            FontFamily = new FontFamily("Segoe UI"),
            FontSize = fontSize,
            FontWeight = fontWeight,
            Foreground = foreground
        };
        Place(page, textBlock, left, top);
    }

    private static void Place(FixedPage page, UIElement element, double left, double top)
    {
        FixedPage.SetLeft(element, left);
        FixedPage.SetTop(element, top);
        page.Children.Add(element);
    }

    private static void TryDelete(string path)
    {
        try { File.Delete(path); }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }

    private enum SmokeScenario { Basic, Multipage, Landscape, Visual }

    private sealed record ScenarioDefinition(string Name, string DocumentName, int PageCount, bool Landscape, bool VisualContent);
}
