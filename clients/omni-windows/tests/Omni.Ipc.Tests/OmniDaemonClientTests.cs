using Omni.Ipc;

namespace Omni.Ipc.Tests;

/// <summary>
/// Exercises <see cref="OmniDaemonClient"/> over a real named pipe against
/// <see cref="FakeDaemon"/>: the actual connect, write-line, read-line transport.
/// </summary>
public class OmniDaemonClientTests
{
    private static CancellationToken Timeout(int seconds = 10) =>
        new CancellationTokenSource(TimeSpan.FromSeconds(seconds)).Token;

    private static OmniDaemonClient ClientFor(FakeDaemon daemon) =>
        new(pipeShortName: daemon.PipeName, connectTimeoutMs: 5000);

    [Fact]
    public async Task Hello_sends_the_handshake_and_parses_the_versions()
    {
        await using var daemon = new FakeDaemon();
        var serve = daemon.ServeRequestAsync(
            _ => "{\"result\":\"hello\",\"protocol_version\":1,\"daemon_version\":\"0.3.7\"}", Timeout());

        var hello = await ClientFor(daemon).HelloAsync(Timeout());
        var request = await serve;

        Assert.Equal("{\"cmd\":\"hello\"}", request);
        Assert.Equal(1, hello.ProtocolVersion);
        Assert.Equal("0.3.7", hello.DaemonVersion);
    }

    [Fact]
    public async Task Status_returns_the_snapshot()
    {
        await using var daemon = new FakeDaemon();
        var serve = daemon.ServeRequestAsync(
            _ => "{\"result\":\"status\",\"fingerprint\":\"ab\",\"port\":4733,\"capturing\":true," +
                 "\"clipboard_sharing\":false,\"sessions\":[],\"pending\":[]}", Timeout());

        var status = await ClientFor(daemon).StatusAsync(Timeout());
        await serve;

        Assert.Equal("ab", status.Fingerprint);
        Assert.Equal(4733, status.Port);
        Assert.True(status.Capturing);
    }

    [Fact]
    public async Task Connect_sends_the_host_and_succeeds_on_ok()
    {
        await using var daemon = new FakeDaemon();
        var serve = daemon.ServeRequestAsync(_ => "{\"result\":\"ok\"}", Timeout());

        await ClientFor(daemon).ConnectAsync("10.0.0.2:4733", Timeout());
        var request = await serve;

        Assert.Equal("{\"cmd\":\"connect\",\"host\":\"10.0.0.2:4733\"}", request);
    }

    [Fact]
    public async Task A_daemon_error_becomes_an_exception_with_the_message()
    {
        await using var daemon = new FakeDaemon();
        var serve = daemon.ServeRequestAsync(_ => "{\"result\":\"error\",\"message\":\"no active session with mac\"}", Timeout());

        var client = ClientFor(daemon);
        var ex = await Assert.ThrowsAsync<OmniDaemonException>(() => client.DisconnectAsync("mac", Timeout()));
        await serve;

        Assert.Equal("no active session with mac", ex.Message);
    }

    [Fact]
    public async Task Subscribe_yields_each_pushed_snapshot()
    {
        await using var daemon = new FakeDaemon();
        var serve = daemon.ServeSubscribeAsync(
        [
            "{\"event\":\"status\",\"fingerprint\":\"ab\",\"port\":4733,\"capturing\":false,\"clipboard_sharing\":false,\"sessions\":[],\"pending\":[]}",
            "{\"event\":\"status\",\"fingerprint\":\"ab\",\"port\":4733,\"capturing\":true,\"clipboard_sharing\":false,\"sessions\":[],\"pending\":[]}",
        ], Timeout());

        var snapshots = new List<StatusInfo>();
        await foreach (var snapshot in ClientFor(daemon).SubscribeAsync(Timeout()))
        {
            snapshots.Add(snapshot);
        }
        var request = await serve;

        Assert.Equal("{\"cmd\":\"subscribe\"}", request);
        Assert.Equal(2, snapshots.Count);
        Assert.False(snapshots[0].Capturing);
        Assert.True(snapshots[1].Capturing);
    }

    [Fact]
    public async Task A_missing_daemon_reports_a_clear_error()
    {
        // No server is listening on this pipe name.
        var client = new OmniDaemonClient(pipeShortName: "omni-test-" + Guid.NewGuid().ToString("N"), connectTimeoutMs: 300);
        var ex = await Assert.ThrowsAsync<OmniDaemonException>(() => client.StatusAsync(Timeout()));
        Assert.Contains("not running", ex.Message);
    }
}
