using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using Omni.Ipc;

namespace Omni.App;

/// <summary>
/// The application's main view. Pure view: it renders <see cref="DaemonViewModel"/>
/// and forwards button clicks to it. No daemon/IPC logic lives here. Hosted by
/// <see cref="MainWindow"/> (a <c>Window</c> is not a <c>FrameworkElement</c>, so
/// x:Bind lives in this <c>UserControl</c>).
/// </summary>
public sealed partial class MainView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public MainView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    private async void OnAcceptClick(object sender, RoutedEventArgs e)
    {
        if (DataContextOf<PendingInfo>(sender) is { } pending)
        {
            await ViewModel.AcceptAsync(pending.Fingerprint);
        }
    }

    private async void OnRejectClick(object sender, RoutedEventArgs e)
    {
        if (DataContextOf<PendingInfo>(sender) is { } pending)
        {
            await ViewModel.RejectAsync(pending.Fingerprint);
        }
    }

    private async void OnDisconnectClick(object sender, RoutedEventArgs e)
    {
        if (DataContextOf<SessionInfo>(sender) is { } session)
        {
            await ViewModel.DisconnectAsync(session.Host);
        }
    }

    private async void OnForgetPeerClick(object sender, RoutedEventArgs e)
    {
        if (DataContextOf<PeerInfo>(sender) is { } peer)
        {
            await ViewModel.RemovePeerAsync(peer.Fingerprint);
        }
    }

    private async void OnConnectClick(object sender, RoutedEventArgs e)
    {
        var host = HostInput.Text.Trim();
        if (host.Length > 0)
        {
            await ViewModel.ConnectAsync(host);
            HostInput.Text = "";
        }
    }

    private async void OnClipboardToggled(object sender, RoutedEventArgs e)
    {
        var toggle = (ToggleSwitch)sender;
        // Only act on a real user change, not the binding echoing a snapshot.
        if (toggle.IsOn != ViewModel.ClipboardSharing)
        {
            await ViewModel.SetClipboardAsync(toggle.IsOn);
        }
    }

    private async void OnSetLayoutClick(object sender, RoutedEventArgs e)
    {
        var host = LayoutHostInput.Text.Trim();
        var edge = (EdgeCombo.SelectedItem as ComboBoxItem)?.Content as string;
        if (host.Length > 0 && !string.IsNullOrEmpty(edge))
        {
            await ViewModel.SetLayoutAsync(host, edge);
            LayoutHostInput.Text = "";
        }
    }

    private static T? DataContextOf<T>(object sender) where T : class =>
        (sender as FrameworkElement)?.DataContext as T;
}
