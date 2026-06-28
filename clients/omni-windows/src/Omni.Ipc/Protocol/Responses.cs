using System.Text.Json.Serialization;
using Omni.Ipc.Json;

namespace Omni.Ipc;

/// <summary>
/// The daemon's answer to an <see cref="OmniRequest"/>. Mirrors the Rust
/// <c>Response</c> enum in <c>omni-runtime/src/ipc.rs</c>: one JSON line tagged by
/// a <c>"result"</c> field. The <c>status</c> variant flattens a
/// <see cref="StatusInfo"/> alongside the tag, so (de)serialization goes through
/// <see cref="OmniResponseConverter"/> rather than plain polymorphism.
/// </summary>
[JsonConverter(typeof(OmniResponseConverter))]
public abstract record OmniResponse;

/// <summary>A command succeeded with nothing else to report.</summary>
public sealed record OkResponse : OmniResponse;

/// <summary>A command failed; <paramref name="Message"/> explains why.</summary>
public sealed record ErrorResponse(string Message) : OmniResponse;

/// <summary>Answer to <see cref="HelloRequest"/>: the daemon's versions.</summary>
public sealed record HelloResponse(int ProtocolVersion, string DaemonVersion) : OmniResponse;

/// <summary>Answer to <see cref="StatusRequest"/>.</summary>
public sealed record StatusResponse(StatusInfo Status) : OmniResponse;

/// <summary>Answer to <see cref="PeersRequest"/>.</summary>
public sealed record PeersResponse(IReadOnlyList<PeerInfo> Peers) : OmniResponse;

/// <summary>Answer to <see cref="LayoutRequest"/> with both fields null.</summary>
public sealed record LayoutResponse(IReadOnlyList<LayoutInfo> Placements) : OmniResponse;
