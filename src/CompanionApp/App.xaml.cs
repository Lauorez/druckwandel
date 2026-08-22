using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.AppLifecycle;
using Windows.Graphics.Printing.Workflow;

namespace ERechnung.CompanionApp;

public partial class App : Application
{
    private Window? window;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        AppActivationArguments activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        window = new Window
        {
            Title = "E-Rechnung – Print Companion"
        };

        var frame = new Frame();
        if (activation.Kind == ExtendedActivationKind.PrintSupportJobUI &&
            activation.Data is PrintWorkflowJobActivatedEventArgs printJobArgs)
        {
            frame.Navigate(typeof(CompanionPage), printJobArgs);
        }
        else if (activation.Kind == ExtendedActivationKind.PrintSupportJobUI)
        {
            frame.Navigate(
                typeof(CompanionPage),
                "PrintSupportJobUI activation data could not be projected. The latest local job is shown.");
        }
        else
        {
            frame.Navigate(typeof(CompanionPage));
        }

        window.Content = frame;
        window.Closed += (_, _) => (frame.Content as CompanionPage)?.CompleteWorkflowDeferral();
        window.Activate();
    }
}
