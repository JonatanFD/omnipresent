using System.IO.Pipes;
using System.Text;

namespace Omni.Ipc.Tests;

/// <summary>
/// An in-process stand-in for the daemon's IPC: a named pipe server with a unique
/// name that serves one connection, so the real <see cref="Omni.Ipc.OmniDaemonClient"/>
/// transport (pipe + JSON lines) is exercised end-to-end without a running daemon.
/// </summary>
internal sealed class FakeDaemon : IAsyncDisposable
{
    private static readonly Encoding Utf8 = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);

    private readonly NamedPipeServerStream _server;

    public FakeDaemon()
    {
        PipeName = "omni-test-" + Guid.NewGuid().ToString("N");
        // Created up front so the constructor's pipe instance exists before the
        // client tries to connect, avoiding a startup race.
        _server = new NamedPipeServerStream(
            PipeName, PipeDirection.InOut, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
    }

    /// <summary>The short pipe name to hand to the client under test.</summary>
    public string PipeName { get; }

    /// <summary>
    /// Accepts one connection, reads the request line, replies with the line that
    /// <paramref name="respond"/> returns, and yields the request that was received.
    /// </summary>
    public async Task<string> ServeRequestAsync(Func<string, string> respond, CancellationToken cancellationToken = default)
    {
        await _server.WaitForConnectionAsync(cancellationToken).ConfigureAwait(false);
        using var reader = new StreamReader(_server, Utf8, false, 1024, leaveOpen: true);
        var request = await reader.ReadLineAsync(cancellationToken).ConfigureAwait(false) ?? "";
        await WriteLineAsync(respond(request), cancellationToken).ConfigureAwait(false);
        return request;
    }

    /// <summary>
    /// Accepts one subscribe connection, pushes each event line in order, then
    /// closes the pipe to end the stream. Returns the subscribe request received.
    /// </summary>
    public async Task<string> ServeSubscribeAsync(IEnumerable<string> eventLines, CancellationToken cancellationToken = default)
    {
        await _server.WaitForConnectionAsync(cancellationToken).ConfigureAwait(false);
        using var reader = new StreamReader(_server, Utf8, false, 1024, leaveOpen: true);
        var request = await reader.ReadLineAsync(cancellationToken).ConfigureAwait(false) ?? "";
        foreach (var line in eventLines)
        {
            await WriteLineAsync(line, cancellationToken).ConfigureAwait(false);
        }
        _server.Disconnect();
        return request;
    }

    private async Task WriteLineAsync(string line, CancellationToken cancellationToken)
    {
        var bytes = Utf8.GetBytes(line + "\n");
        await _server.WriteAsync(bytes, cancellationToken).ConfigureAwait(false);
        await _server.FlushAsync(cancellationToken).ConfigureAwait(false);
    }

    public ValueTask DisposeAsync()
    {
        _server.Dispose();
        return ValueTask.CompletedTask;
    }
}
