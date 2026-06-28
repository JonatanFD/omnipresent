using Omni.App.Core;
using Omni.Ipc;

namespace Omni.App.Tests;

public class DaemonViewModelTests
{
    private static StatusInfo Snapshot(bool capturing = false, bool clipboard = false) =>
        new("ab".PadRight(4, 'c'), 4733, capturing, clipboard,
            [new SessionInfo("mac", "cd", "controller", true)],
            [new PendingInfo("win", "ef")]);

    private static DaemonViewModel NewViewModel(FakeDaemonClient client) =>
        new(client, new ImmediateDispatcher(), reconnectDelay: TimeSpan.FromMinutes(5));

    [Fact]
    public async Task Run_applies_pushed_snapshots_and_goes_connected()
    {
        var client = new FakeDaemonClient();
        client.Snapshots.Add(Snapshot(capturing: false));
        client.Snapshots.Add(Snapshot(capturing: true));
        client.PeersList.Add(new PeerInfo("mac", "cd", true));
        client.Placements.Add(new LayoutInfo("mac", "left", true));

        using var cts = new CancellationTokenSource();
        // Stop the reconnect loop once the stream has been fully consumed.
        client.OnSubscribeDrained = cts.Cancel;

        var vm = NewViewModel(client);
        await vm.RunAsync(cts.Token);

        Assert.Equal(ConnectionStatus.Connected, vm.Connection);
        Assert.True(vm.IsConnected);
        Assert.Equal(4733, vm.Port);
        Assert.True(vm.Capturing); // from the last snapshot
        Assert.Equal("0.3.7", vm.DaemonVersion);
        Assert.Equal("mac", Assert.Single(vm.Sessions).Host);
        Assert.Equal("win", Assert.Single(vm.Pending).Host);
        Assert.Equal("mac", Assert.Single(vm.Peers).Host);
        Assert.Equal("left", Assert.Single(vm.Placements).Edge);
    }

    [Fact]
    public async Task A_newer_daemon_protocol_is_reported_as_incompatible()
    {
        var client = new FakeDaemonClient { Hello = new HelloResponse(OmniProtocol.Version + 1, "9.9.9") };

        var vm = NewViewModel(client);
        await vm.RunAsync(CancellationToken.None); // returns immediately, no retry

        Assert.Equal(ConnectionStatus.Incompatible, vm.Connection);
        Assert.False(vm.IsConnected);
        Assert.Contains("Please update", vm.LastError);
        Assert.Empty(client.Calls);
    }

    [Fact]
    public async Task A_missing_daemon_goes_disconnected_and_keeps_retrying()
    {
        // Hello throws (daemon down) the first time, then we cancel so the loop ends.
        var client = new ThrowingThenStopClient();
        var vm = new DaemonViewModel(client, new ImmediateDispatcher(), reconnectDelay: TimeSpan.FromMinutes(5));

        await vm.RunAsync(client.Token);

        Assert.Equal(ConnectionStatus.Disconnected, vm.Connection);
        Assert.True(client.Attempts >= 1);
    }

    [Theory]
    [InlineData("accept", "ab12")]
    [InlineData("reject", "ab12")]
    [InlineData("connect", "10.0.0.2:4733")]
    [InlineData("remove_peer", "laptop")]
    public async Task Commands_forward_to_the_client(string command, string argument)
    {
        var client = new FakeDaemonClient();
        var vm = NewViewModel(client);

        Task call = command switch
        {
            "accept" => vm.AcceptAsync(argument),
            "reject" => vm.RejectAsync(argument),
            "connect" => vm.ConnectAsync(argument),
            "remove_peer" => vm.RemovePeerAsync(argument),
            _ => throw new ArgumentOutOfRangeException(nameof(command)),
        };
        await call;

        Assert.Contains((command, argument), client.Calls);
        Assert.Null(vm.LastError);
    }

    [Fact]
    public async Task Layout_and_clipboard_commands_forward_their_arguments()
    {
        var client = new FakeDaemonClient();
        var vm = NewViewModel(client);

        await vm.SetLayoutAsync("mac", "right");
        await vm.SetClipboardAsync(true);

        Assert.Contains(("layout", "mac:right"), client.Calls);
        Assert.Contains(("clipboard", "on"), client.Calls);
    }

    [Fact]
    public async Task A_failed_command_surfaces_its_message_in_last_error()
    {
        var client = new FakeDaemonClient { FailDisconnectWith = "no active session with mac" };
        var vm = NewViewModel(client);

        await vm.DisconnectAsync("mac");

        Assert.Equal("no active session with mac", vm.LastError);
    }

    /// <summary>A client whose handshake always fails, cancelling after one attempt.</summary>
    private sealed class ThrowingThenStopClient : IOmniDaemonClient
    {
        private readonly CancellationTokenSource _cts = new();
        public int Attempts { get; private set; }
        public CancellationToken Token => _cts.Token;

        public Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default)
        {
            Attempts++;
            _cts.Cancel(); // end the retry loop after this attempt
            throw new OmniDaemonException("the omni daemon is not running");
        }

        public Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task ConnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task DisconnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task AcceptAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RejectAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task StopAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public IAsyncEnumerable<StatusInfo> SubscribeAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
    }
}
