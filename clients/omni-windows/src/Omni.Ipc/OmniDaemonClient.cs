using System.IO.Pipes;
using System.Runtime.CompilerServices;
using System.Text;
using System.Text.Json;
using Omni.Ipc.Json;

namespace Omni.Ipc;

/// <summary>
/// The default <see cref="IOmniDaemonClient"/>: speaks JSON lines over the
/// daemon's Windows named pipe. Each command opens a short-lived connection (like
/// the <c>omni</c> CLI); <see cref="SubscribeAsync"/> keeps one open for the push
/// stream.
/// </summary>
public sealed class OmniDaemonClient : IOmniDaemonClient
{
    private static readonly Encoding Utf8NoBom = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);

    private readonly string _serverName;
    private readonly string _pipeName;
    private readonly int _connectTimeoutMs;

    /// <param name="pipeShortName">
    /// Pipe name without the <c>\\.\pipe\</c> prefix; defaults to the running
    /// daemon's pipe (see <see cref="OmniPaths.PipeShortName"/>).
    /// </param>
    /// <param name="serverName">Pipe host; "." (this machine) by default.</param>
    /// <param name="connectTimeoutMs">How long to wait for the daemon to answer a connect.</param>
    public OmniDaemonClient(string? pipeShortName = null, string serverName = ".", int connectTimeoutMs = 2000)
    {
        _pipeName = pipeShortName ?? OmniPaths.PipeShortName();
        _serverName = serverName;
        _connectTimeoutMs = connectTimeoutMs;
    }

    public async Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default) =>
        await SendAsync(new HelloRequest(), cancellationToken).ConfigureAwait(false) switch
        {
            HelloResponse hello => hello,
            ErrorResponse error => throw new OmniDaemonException(error.Message),
            var other => throw Unexpected(other),
        };

    public async Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) =>
        await SendAsync(new StatusRequest(), cancellationToken).ConfigureAwait(false) switch
        {
            StatusResponse status => status.Status,
            ErrorResponse error => throw new OmniDaemonException(error.Message),
            var other => throw Unexpected(other),
        };

    public async Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) =>
        await SendAsync(new PeersRequest(), cancellationToken).ConfigureAwait(false) switch
        {
            PeersResponse peers => peers.Peers,
            ErrorResponse error => throw new OmniDaemonException(error.Message),
            var other => throw Unexpected(other),
        };

    public async Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) =>
        await SendAsync(new LayoutRequest(null, null), cancellationToken).ConfigureAwait(false) switch
        {
            LayoutResponse layout => layout.Placements,
            ErrorResponse error => throw new OmniDaemonException(error.Message),
            var other => throw Unexpected(other),
        };

    public Task ConnectAsync(string host, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new ConnectRequest(host), cancellationToken);

    public Task DisconnectAsync(string host, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new DisconnectRequest(host), cancellationToken);

    public Task AcceptAsync(string selector, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new AcceptRequest(selector), cancellationToken);

    public Task RejectAsync(string selector, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new RejectRequest(selector), cancellationToken);

    public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new RemovePeerRequest(selector), cancellationToken);

    public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new LayoutRequest(host, edge), cancellationToken);

    public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new ClipboardRequest(enabled), cancellationToken);

    public Task StopAsync(CancellationToken cancellationToken = default) =>
        ExpectOkAsync(new StopRequest(), cancellationToken);

    public async IAsyncEnumerable<StatusInfo> SubscribeAsync(
        [EnumeratorCancellation] CancellationToken cancellationToken = default)
    {
        await using var pipe = await ConnectPipeAsync(cancellationToken).ConfigureAwait(false);
        await WriteLineAsync(pipe, new SubscribeRequest(), cancellationToken).ConfigureAwait(false);

        using var reader = new StreamReader(pipe, Utf8NoBom, detectEncodingFromByteOrderMarks: false, bufferSize: 1024, leaveOpen: true);
        while (!cancellationToken.IsCancellationRequested)
        {
            string? line;
            try
            {
                line = await reader.ReadLineAsync(cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                yield break;
            }
            catch (IOException)
            {
                yield break; // the daemon went away
            }

            if (line is null)
            {
                yield break; // end of stream
            }
            if (line.Length == 0)
            {
                continue;
            }

            StatusInfo? snapshot = JsonSerializer.Deserialize<OmniEvent>(line, OmniJson.Options) switch
            {
                StatusEvent statusEvent => statusEvent.Status,
                _ => null,
            };
            if (snapshot is not null)
            {
                yield return snapshot;
            }
        }
    }

    private async Task ExpectOkAsync(OmniRequest request, CancellationToken cancellationToken)
    {
        switch (await SendAsync(request, cancellationToken).ConfigureAwait(false))
        {
            case OkResponse:
                return;
            case ErrorResponse error:
                throw new OmniDaemonException(error.Message);
            case var other:
                throw Unexpected(other);
        }
    }

    private async Task<OmniResponse> SendAsync(OmniRequest request, CancellationToken cancellationToken)
    {
        await using var pipe = await ConnectPipeAsync(cancellationToken).ConfigureAwait(false);
        await WriteLineAsync(pipe, request, cancellationToken).ConfigureAwait(false);

        using var reader = new StreamReader(pipe, Utf8NoBom, detectEncodingFromByteOrderMarks: false, bufferSize: 1024, leaveOpen: true);
        var line = await reader.ReadLineAsync(cancellationToken).ConfigureAwait(false)
                   ?? throw new OmniDaemonException("the daemon closed the connection without responding");

        try
        {
            return JsonSerializer.Deserialize<OmniResponse>(line, OmniJson.Options)
                   ?? throw new OmniDaemonException("the daemon sent an empty response");
        }
        catch (JsonException e)
        {
            throw new OmniDaemonException("the daemon sent a response this client cannot read", e);
        }
    }

    private async Task<NamedPipeClientStream> ConnectPipeAsync(CancellationToken cancellationToken)
    {
        var pipe = new NamedPipeClientStream(_serverName, _pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
        try
        {
            await pipe.ConnectAsync(_connectTimeoutMs, cancellationToken).ConfigureAwait(false);
            return pipe;
        }
        catch (TimeoutException)
        {
            await pipe.DisposeAsync().ConfigureAwait(false);
            throw new OmniDaemonException("the omni daemon is not running (no pipe to connect to)");
        }
        catch (Exception) when (cancellationToken.IsCancellationRequested)
        {
            await pipe.DisposeAsync().ConfigureAwait(false);
            throw;
        }
        catch (IOException e)
        {
            await pipe.DisposeAsync().ConfigureAwait(false);
            throw new OmniDaemonException("could not reach the omni daemon", e);
        }
    }

    private static async Task WriteLineAsync(Stream stream, OmniRequest request, CancellationToken cancellationToken)
    {
        // Serialize as the base type so the polymorphic "cmd" tag is written.
        var json = JsonSerializer.Serialize(request, OmniJson.Options);
        var bytes = Utf8NoBom.GetBytes(json + "\n");
        await stream.WriteAsync(bytes, cancellationToken).ConfigureAwait(false);
        await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
    }

    private static OmniDaemonException Unexpected(OmniResponse response) =>
        new($"unexpected response from the daemon: {response.GetType().Name}");
}
