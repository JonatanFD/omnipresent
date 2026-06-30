using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using System.Diagnostics;

namespace Omni.App.Views;

public sealed partial class UpdateView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public UpdateView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    private async void OnUpdateClick(object sender, RoutedEventArgs e)
    {
        UpdateButton.IsEnabled = false;
        UpdateProgress.IsActive = true;
        UpdateMessage.Visibility = Visibility.Collapsed;

        var message = await RunUpdateAsync();

        UpdateMessage.Text = message;
        UpdateMessage.Visibility = Visibility.Visible;
        UpdateProgress.IsActive = false;
        UpdateButton.IsEnabled = true;
    }

    private Task<string> RunUpdateAsync()
    {
        return Task.Run(() =>
        {
            var candidates = new[]
            {
                @"C:\Program Files\Omnipresent\omni.exe",
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".cargo", "bin", "omni.exe"),
            };

            var binaryPath = candidates.FirstOrDefault(p => File.Exists(p));
            if (binaryPath is null)
            {
                return "Could not find the omni binary. Make sure Omnipresent is installed.";
            }

            try
            {
                var process = new Process
                {
                    StartInfo = new ProcessStartInfo
                    {
                        FileName = binaryPath,
                        Arguments = "update",
                        UseShellExecute = false,
                        CreateNoWindow = true,
                    }
                };

                process.Start();
                process.WaitForExit();

                return process.ExitCode == 0
                    ? "Update complete. The daemon will restart shortly."
                    : $"Update exited with code {process.ExitCode}.";
            }
            catch (Exception ex)
            {
                return $"Failed to run update: {ex.Message}";
            }
        });
    }
}
