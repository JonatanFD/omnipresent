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

    protected override void OnLoaded()
    {
        base.OnLoaded();
        UpdateStatusIcon();

        // Update icon when connection status changes
        if (ViewModel is INotifyPropertyChanged observable)
        {
            observable.PropertyChanged += (_, e) =>
            {
                if (e.PropertyName == nameof(ViewModel.Connection) || e.PropertyName == nameof(ViewModel.IsIncompatible))
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
            DaemonViewModel.ConnectionStatus.Connected => Symbol.Accept,
            DaemonViewModel.ConnectionStatus.Connecting => Symbol.WaitUntilDone,
            DaemonViewModel.ConnectionStatus.Disconnected => Symbol.Block,
            DaemonViewModel.ConnectionStatus.Incompatible => Symbol.Warning,
            _ => Symbol.Help,
        };
    }

    private void UpdatePanelVisibility()
    {
        DaemonControlPanel.Visibility = ViewModel.IsIncompatible ? Visibility.Collapsed : Visibility.Visible;
    }
}
