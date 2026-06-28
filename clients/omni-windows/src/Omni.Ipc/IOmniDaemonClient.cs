namespace Omni.Ipc;

/// <summary>
/// A thin client of the daemon's local IPC. Every method maps to one
/// <see cref="OmniRequest"/> / <see cref="OmniResponse"/> exchange, except
/// <see cref="SubscribeAsync"/>, which keeps a connection open and yields a fresh
/// snapshot on every change. The client carries no business logic — the daemon
/// owns all state.
/// </summary>
public interface IOmniDaemonClient
{
    /// <summary>Handshake; returns the daemon's protocol and build versions.</summary>
    Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default);

    /// <summary>A one-shot status snapshot.</summary>
    Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default);

    /// <summary>Known peers and their state.</summary>
    Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default);

    /// <summary>Where each peer sits in the virtual desktop.</summary>
    Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default);

    /// <summary>Dial a peer and request control of it.</summary>
    Task ConnectAsync(string host, CancellationToken cancellationToken = default);

    /// <summary>End the session with a peer.</summary>
    Task DisconnectAsync(string host, CancellationToken cancellationToken = default);

    /// <summary>Approve a pending incoming request (host or fingerprint prefix).</summary>
    Task AcceptAsync(string selector, CancellationToken cancellationToken = default);

    /// <summary>Deny a pending incoming request (host or fingerprint prefix).</summary>
    Task RejectAsync(string selector, CancellationToken cancellationToken = default);

    /// <summary>Forget a peer (host or fingerprint prefix).</summary>
    Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default);

    /// <summary>Place a peer past the given edge: "left", "right", "top", "bottom".</summary>
    Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default);

    /// <summary>Turn opt-in clipboard sharing on or off.</summary>
    Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default);

    /// <summary>Ask the daemon to shut down.</summary>
    Task StopAsync(CancellationToken cancellationToken = default);

    /// <summary>
    /// Subscribe to live updates. Yields the current status immediately, then a
    /// fresh snapshot every time daemon state changes, until cancelled or the
    /// daemon goes away.
    /// </summary>
    IAsyncEnumerable<StatusInfo> SubscribeAsync(CancellationToken cancellationToken = default);
}
