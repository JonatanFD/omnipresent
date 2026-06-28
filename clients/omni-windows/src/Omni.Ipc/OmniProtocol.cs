namespace Omni.Ipc;

/// <summary>
/// Constants shared with the daemon's IPC surface (see
/// <c>crates/omni-runtime/src/ipc.rs</c>). Kept in sync with the Rust source of
/// truth by hand for now; a future step generates these types from Rust.
/// </summary>
public static class OmniProtocol
{
    /// <summary>
    /// The IPC protocol version this client understands. A client sends
    /// <see cref="HelloRequest"/> first and compares the daemon's reported version;
    /// if the daemon is newer, the client should tell the user to update rather
    /// than misbehave.
    /// </summary>
    public const int Version = 1;
}
