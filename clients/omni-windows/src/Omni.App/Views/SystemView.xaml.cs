using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;

namespace Omni.App.Views;

public sealed partial class SystemView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public SystemView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    private async void OnClipboardToggled(object sender, RoutedEventArgs e)
    {
        var toggle = (ToggleSwitch)sender;
        if (toggle.IsOn != ViewModel.ClipboardSharing)
        {
            await ViewModel.SetClipboardAsync(toggle.IsOn);
        }
    }
}
