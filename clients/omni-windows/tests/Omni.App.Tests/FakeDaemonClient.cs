using System.Runtime.CompilerServices;
using Omni.Ipc;

namespace Omni.App.Tests;

/// <summary>
/// An in-memory <see cref="IOmniDaemonClient"/> for view-model tests: canned
/// responses, a scripted subscribe stream, and a record of the commands it was
/// asked to run — no pipes, no daemon, deterministic.
/// </summary>
internal sealed class FakeDaemonClient : IOmniDaemonClient
{
    public HelloResponse Hello { get; set; } = new(OmniProtocol.Version, "0.3.7");

    /// <summary>Snapshots the subscribe stream yields, in order, then completes.</summary>
    public List<StatusInfo> Snapshots { get; } = [];

    public List<PeerInfo> PeersList { get; } = [];
    public List<LayoutInfo> Placements { get; } = [];

    /// <summary>Invoked once the subscribe stream has yielded every snapshot.</summary>
    public Action? OnSubscribeDrained { get; set; }

    /// <summary>Every command the view model issued, as (name, argument) pairs.</summary>
    public List<(string Command, string Argument)> Calls { get; } = [];

    /// <summary>If set, the matching command throws this as a daemon error.</summary>
    public string? FailDisconnectWith { get; set; }

    public Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(Hello);

    public Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(Snapshots[^1]);

    public Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult<IReadOnlyList<PeerInfo>>(PeersList);

    public Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult<IReadOnlyList<LayoutInfo>>(Placements);

    public Task ConnectAsync(string host, CancellationToken cancellationToken = default)
    {
        Calls.Add(("connect", host));
        return Task.CompletedTask;
    }

    public Task DisconnectAsync(string host, CancellationToken cancellationToken = default)
    {
        Calls.Add(("disconnect", host));
        return FailDisconnectWith is { } message
            ? Task.FromException(new OmniDaemonException(message))
            : Task.CompletedTask;
    }

    public Task AcceptAsync(string selector, CancellationToken cancellationToken = default)
    {
        Calls.Add(("accept", selector));
        return Task.CompletedTask;
    }

    public Task RejectAsync(string selector, CancellationToken cancellationToken = default)
    {
        Calls.Add(("reject", selector));
        return Task.CompletedTask;
    }

    public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default)
    {
        Calls.Add(("remove_peer", selector));
        return Task.CompletedTask;
    }

    public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default)
    {
        Calls.Add(("layout", $"{host}:{edge}"));
        return Task.CompletedTask;
    }

    public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default)
    {
        Calls.Add(("clipboard", enabled ? "on" : "off"));
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken cancellationToken = default)
    {
        Calls.Add(("stop", ""));
        return Task.CompletedTask;
    }

    public async IAsyncEnumerable<StatusInfo> SubscribeAsync(
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await Task.Yield();
        foreach (var snapshot in Snapshots)
        {
            cancellationToken.ThrowIfCancellationRequested();
            yield return snapshot;
        }
        OnSubscribeDrained?.Invoke();
    }
}
