using System.Collections.ObjectModel;
using Omni.Ipc;

namespace Omni.App.Core;

/// <summary>
/// The app's main view model: a live, bindable view of the daemon. It performs the
/// version handshake, follows the daemon's push stream to keep state current,
/// reconnects on its own when the daemon goes away, and exposes commands that map
/// straight to IPC requests. It holds no business logic — the daemon owns state.
/// </summary>
public sealed class DaemonViewModel : ObservableObject
{
    private readonly IOmniDaemonClient _client;
    private readonly IUiDispatcher _ui;
    private readonly TimeSpan _reconnectDelay;

    public DaemonViewModel(IOmniDaemonClient client, IUiDispatcher ui, TimeSpan? reconnectDelay = null)
    {
        _client = client;
        _ui = ui;
        _reconnectDelay = reconnectDelay ?? TimeSpan.FromSeconds(2);

        // Keep the "has any …" flags (used to show/hide sections) in step with the
        // collections; these run on the UI thread, inside the same update.
        Pending.CollectionChanged += (_, _) => Raise(nameof(HasPending));
        Sessions.CollectionChanged += (_, _) => Raise(nameof(HasSessions));
        Peers.CollectionChanged += (_, _) => Raise(nameof(HasPeers));
        Placements.CollectionChanged += (_, _) => Raise(nameof(HasPlacements));
    }

    private ConnectionStatus _connection = ConnectionStatus.Connecting;
    public ConnectionStatus Connection
    {
        get => _connection;
        private set
        {
            if (SetField(ref _connection, value))
            {
                Raise(nameof(IsConnected));
                Raise(nameof(IsIncompatible));
            }
        }
    }

    /// <summary>True only while live, for enabling/disabling controls.</summary>
    public bool IsConnected => _connection == ConnectionStatus.Connected;

    /// <summary>True when the daemon is too new; the UI shows an "update" notice.</summary>
    public bool IsIncompatible => _connection == ConnectionStatus.Incompatible;

    private string _statusText = "Connecting…";
    public string StatusText { get => _statusText; private set => SetField(ref _statusText, value); }

    private string _fingerprint = "";
    public string Fingerprint { get => _fingerprint; private set => SetField(ref _fingerprint, value); }

    private int _port;
    public int Port { get => _port; private set => SetField(ref _port, value); }

    private bool _capturing;
    public bool Capturing { get => _capturing; private set => SetField(ref _capturing, value); }

    private bool _clipboardSharing;
    public bool ClipboardSharing { get => _clipboardSharing; private set => SetField(ref _clipboardSharing, value); }

    private string _daemonVersion = "";
    public string DaemonVersion { get => _daemonVersion; private set => SetField(ref _daemonVersion, value); }

    private string? _lastError;
    public string? LastError
    {
        get => _lastError;
        private set
        {
            if (SetField(ref _lastError, value))
            {
                Raise(nameof(HasError));
            }
        }
    }

    /// <summary>True when there is an error message to show.</summary>
    public bool HasError => !string.IsNullOrEmpty(_lastError);

    public ObservableCollection<SessionInfo> Sessions { get; } = [];
    public ObservableCollection<PendingInfo> Pending { get; } = [];
    public ObservableCollection<PeerInfo> Peers { get; } = [];
    public ObservableCollection<LayoutInfo> Placements { get; } = [];

    public bool HasPending => Pending.Count > 0;
    public bool HasSessions => Sessions.Count > 0;
    public bool HasPeers => Peers.Count > 0;
    public bool HasPlacements => Placements.Count > 0;

    /// <summary>
    /// Runs until <paramref name="cancellationToken"/> is cancelled: handshake,
    /// then follow the push stream, reconnecting after a delay whenever it drops.
    /// Stops permanently only if the daemon is too new (see
    /// <see cref="ConnectionStatus.Incompatible"/>).
    /// </summary>
    public async Task RunAsync(CancellationToken cancellationToken)
    {
        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                var hello = await _client.HelloAsync(cancellationToken).ConfigureAwait(false);
                if (hello.ProtocolVersion > OmniProtocol.Version)
                {
                    SetState(ConnectionStatus.Incompatible,
                        $"The daemon speaks protocol v{hello.ProtocolVersion}; this app understands v{OmniProtocol.Version}. Please update Omnipresent.");
                    return;
                }
                _ui.Post(() => DaemonVersion = hello.DaemonVersion);

                await foreach (var snapshot in _client.SubscribeAsync(cancellationToken).ConfigureAwait(false))
                {
                    Apply(snapshot);
                    await RefreshListsAsync(cancellationToken).ConfigureAwait(false);
                    SetState(ConnectionStatus.Connected, "Connected");
                }
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (OmniDaemonException ex)
            {
                SetState(ConnectionStatus.Disconnected, ex.Message);
            }

            if (cancellationToken.IsCancellationRequested)
            {
                break;
            }
            SetState(ConnectionStatus.Disconnected, "Waiting for the daemon…");
            try
            {
                await Task.Delay(_reconnectDelay, cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    public Task ConnectAsync(string host) => Guard(ct => _client.ConnectAsync(host, ct));
    public Task DisconnectAsync(string host) => Guard(ct => _client.DisconnectAsync(host, ct));
    public Task AcceptAsync(string selector) => Guard(ct => _client.AcceptAsync(selector, ct));
    public Task RejectAsync(string selector) => Guard(ct => _client.RejectAsync(selector, ct));
    public Task RemovePeerAsync(string selector) => Guard(ct => _client.RemovePeerAsync(selector, ct));
    public Task SetLayoutAsync(string host, string edge) => Guard(ct => _client.SetLayoutAsync(host, edge, ct));
    public Task SetClipboardAsync(bool enabled) => Guard(ct => _client.SetClipboardAsync(enabled, ct));

    private async Task Guard(Func<CancellationToken, Task> action)
    {
        _ui.Post(() => LastError = null);
        try
        {
            await action(CancellationToken.None).ConfigureAwait(false);
        }
        catch (OmniDaemonException ex)
        {
            _ui.Post(() => LastError = ex.Message);
        }
    }

    private void Apply(StatusInfo snapshot) => _ui.Post(() =>
    {
        Fingerprint = snapshot.Fingerprint;
        Port = snapshot.Port;
        Capturing = snapshot.Capturing;
        ClipboardSharing = snapshot.ClipboardSharing;
        Replace(Sessions, snapshot.Sessions);
        Replace(Pending, snapshot.Pending);
    });

    private async Task RefreshListsAsync(CancellationToken cancellationToken)
    {
        // Peers and placements are separate requests, not part of the snapshot;
        // refresh them whenever state changes so the lists stay live.
        try
        {
            var peers = await _client.PeersAsync(cancellationToken).ConfigureAwait(false);
            var placements = await _client.LayoutAsync(cancellationToken).ConfigureAwait(false);
            _ui.Post(() =>
            {
                Replace(Peers, peers);
                Replace(Placements, placements);
            });
        }
        catch (OmniDaemonException)
        {
            // Keep whatever we last had; the snapshot itself still applied.
        }
    }

    private void SetState(ConnectionStatus status, string text) => _ui.Post(() =>
    {
        Connection = status;
        StatusText = text;
        if (status == ConnectionStatus.Incompatible)
        {
            LastError = text;
        }
    });

    private static void Replace<T>(ObservableCollection<T> target, IReadOnlyList<T> items)
    {
        target.Clear();
        foreach (var item in items)
        {
            target.Add(item);
        }
    }
}
