using System.Text.Json.Serialization;
using Omni.Ipc.Json;

namespace Omni.Ipc;

/// <summary>
/// A pushed update sent on a <see cref="SubscribeRequest"/> connection. Mirrors
/// the Rust <c>Event</c> enum in <c>omni-runtime/src/ipc.rs</c>: one JSON line
/// tagged by an <c>"event"</c> field, which is what lets a subscriber tell a push
/// apart from an <see cref="OmniResponse"/>.
/// </summary>
[JsonConverter(typeof(OmniEventConverter))]
public abstract record OmniEvent;

/// <summary>A fresh full status snapshot; the client just re-renders.</summary>
public sealed record StatusEvent(StatusInfo Status) : OmniEvent;
