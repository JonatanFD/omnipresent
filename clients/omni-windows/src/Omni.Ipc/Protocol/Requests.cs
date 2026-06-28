using System.Text.Json.Serialization;

namespace Omni.Ipc;

/// <summary>
/// A command sent from a client to the daemon. Mirrors the Rust <c>Request</c>
/// enum in <c>omni-runtime/src/ipc.rs</c>: one JSON line tagged by a <c>"cmd"</c>
/// field, e.g. <c>{"cmd":"connect","host":"10.0.0.2:4733"}</c>.
/// </summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "cmd")]
[JsonDerivedType(typeof(HelloRequest), "hello")]
[JsonDerivedType(typeof(SubscribeRequest), "subscribe")]
[JsonDerivedType(typeof(StatusRequest), "status")]
[JsonDerivedType(typeof(StopRequest), "stop")]
[JsonDerivedType(typeof(ConnectRequest), "connect")]
[JsonDerivedType(typeof(DisconnectRequest), "disconnect")]
[JsonDerivedType(typeof(AcceptRequest), "accept")]
[JsonDerivedType(typeof(RejectRequest), "reject")]
[JsonDerivedType(typeof(PeersRequest), "peers")]
[JsonDerivedType(typeof(RemovePeerRequest), "remove_peer")]
[JsonDerivedType(typeof(LayoutRequest), "layout")]
[JsonDerivedType(typeof(ClipboardRequest), "clipboard")]
public abstract record OmniRequest;

/// <summary>Version handshake: ask which protocol version the daemon speaks.</summary>
public sealed record HelloRequest : OmniRequest;

/// <summary>Subscribe to live updates; the connection then streams events.</summary>
public sealed record SubscribeRequest : OmniRequest;

/// <summary>Daemon and session overview.</summary>
public sealed record StatusRequest : OmniRequest;

/// <summary>Shut the daemon down.</summary>
public sealed record StopRequest : OmniRequest;

/// <summary>Dial a peer and request control of it.</summary>
public sealed record ConnectRequest(string Host) : OmniRequest;

/// <summary>End the session with a peer.</summary>
public sealed record DisconnectRequest(string Host) : OmniRequest;

/// <summary>Approve a pending incoming request (by host or fingerprint prefix).</summary>
public sealed record AcceptRequest(string Selector) : OmniRequest;

/// <summary>Deny a pending incoming request (by host or fingerprint prefix).</summary>
public sealed record RejectRequest(string Selector) : OmniRequest;

/// <summary>List known peers.</summary>
public sealed record PeersRequest : OmniRequest;

/// <summary>Forget a peer (by host or fingerprint prefix).</summary>
public sealed record RemovePeerRequest(string Selector) : OmniRequest;

/// <summary>
/// Inspect or change where peers sit in the virtual desktop. With both
/// <paramref name="Host"/> and <paramref name="Edge"/> set, place that peer past
/// the given edge; with both null, list the current placements.
/// </summary>
public sealed record LayoutRequest(string? Host, string? Edge) : OmniRequest;

/// <summary>Turn opt-in clipboard sharing on or off at runtime.</summary>
public sealed record ClipboardRequest(bool Enabled) : OmniRequest;
