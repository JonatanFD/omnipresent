using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using Omni.Ipc;

namespace Omni.App.Views;

public sealed partial class ConnectionsView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public ConnectionsView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    public bool IsEmpty => !ViewModel.IsConnected && !ViewModel.HasSessions && !ViewModel.HasPending && !ViewModel.HasPeers && !ViewModel.HasPlacements;
    public bool IsNotEmpty => !IsEmpty;

    private async void OnConnectClick(object sender, RoutedEventArgs e)
    {
        var host = HostInput.Text.Trim();
        if (host.Length > 0)
        {
            await ViewModel.ConnectAsync(host);
            HostInput.Text = "";
        }
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
