using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
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
        // Find the PendingRequestCard and get its Request
        if (FindComponentByTag<Components.PendingRequestCard>(sender as FrameworkElement) is { } card
            && card.Request is { } pending)
        {
            await ViewModel.AcceptAsync(pending.Fingerprint);
        }
    }

    private async void OnRejectClick(object sender, RoutedEventArgs e)
    {
        if (FindComponentByTag<Components.PendingRequestCard>(sender as FrameworkElement) is { } card
            && card.Request is { } pending)
        {
            await ViewModel.RejectAsync(pending.Fingerprint);
        }
    }

    private async void OnDisconnectClick(object sender, RoutedEventArgs e)
    {
        if (FindComponentByTag<Components.SessionCard>(sender as FrameworkElement) is { } card
            && card.Session is { } session)
        {
            await ViewModel.DisconnectAsync(session.Host);
        }
    }

    private async void OnForgetPeerClick(object sender, RoutedEventArgs e)
    {
        if (FindComponentByTag<Components.PeerCard>(sender as FrameworkElement) is { } card
            && card.Peer is { } peer)
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

    private static T? FindComponentByTag<T>(FrameworkElement? element) where T : class
    {
        // Walk up the visual tree to find a component of type T
        while (element != null)
        {
            if (element is T component)
                return component;
            element = VisualTreeHelper.GetParent(element) as FrameworkElement;
        }
        return null;
    }
}
