using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using System.ComponentModel;

namespace Omni.App.Views;

public sealed partial class GeneralView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public GeneralView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
        Loaded += OnLoaded;
    }

    public string CaptureStatus => ViewModel.Capturing ? "Active" : "Target only";

    private async void OnStartClick(object sender, RoutedEventArgs e)
    {
        await ViewModel.StartDaemonAsync();
    }

    private async void OnStopClick(object sender, RoutedEventArgs e)
    {
        await ViewModel.StopDaemonAsync();
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        UpdateStatusIcon();

        // Update icon when connection status changes
        if (ViewModel is INotifyPropertyChanged observable)
        {
            observable.PropertyChanged += (_, args) =>
            {
                if (args.PropertyName == nameof(ViewModel.Connection) || args.PropertyName == nameof(ViewModel.IsIncompatible))
                {
                    UpdateStatusIcon();
                    UpdatePanelVisibility();
                }
            };
        }
    }

    private void UpdateStatusIcon()
    {
        StatusIcon.Symbol = ViewModel.Connection switch
        {
            ConnectionStatus.Connected => Symbol.Accept,
            ConnectionStatus.Connecting => Symbol.Sync,
            ConnectionStatus.Disconnected => Symbol.Cancel,
            ConnectionStatus.Incompatible => Symbol.Important,
            _ => Symbol.Help,
        };
    }

    private void UpdatePanelVisibility()
    {
        DaemonControlPanel.Visibility = ViewModel.IsIncompatible ? Visibility.Collapsed : Visibility.Visible;
    }
}
