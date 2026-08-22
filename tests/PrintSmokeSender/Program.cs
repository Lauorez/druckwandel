using System.Drawing;
using System.Drawing.Printing;

const string printerName = "E-Rechnung";
const string documentName = "E-Rechnung automated smoke test";

using var document = new PrintDocument
{
    DocumentName = documentName,
    PrintController = new StandardPrintController()
};

document.PrinterSettings.PrinterName = printerName;
if (!document.PrinterSettings.IsValid)
{
    throw new InvalidOperationException($"Printer '{printerName}' is not available.");
}

document.DefaultPageSettings.PaperSize = new PaperSize("A4", 827, 1169);
document.DefaultPageSettings.Landscape = false;
document.PrintPage += (_, args) =>
{
    Graphics graphics = args.Graphics
        ?? throw new InvalidOperationException("The print controller did not provide a graphics surface.");
    using var titleFont = new Font("Segoe UI", 18, FontStyle.Bold);
    using var bodyFont = new Font("Segoe UI", 11);
    using var pen = new Pen(Color.DarkBlue, 2);

    graphics.DrawString("E-Rechnung Virtual Printer", titleFont, Brushes.DarkBlue, 80, 80);
    graphics.DrawString(
        "Automated local print-pipeline smoke test.\nNo invoice processing is performed.",
        bodyFont,
        Brushes.Black,
        80,
        140);
    graphics.DrawRectangle(pen, 80, 220, 500, 180);
    graphics.DrawString(DateTimeOffset.UtcNow.ToString("O"), bodyFont, Brushes.Black, 100, 250);
    args.HasMorePages = false;
};

document.Print();
Console.WriteLine($"Submitted '{documentName}' to '{printerName}'.");
