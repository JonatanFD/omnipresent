namespace Omni.Ipc;

/// <summary>
/// What <c>omni status</c> shows. Mirrors the Rust <c>StatusInfo</c> struct; JSON
/// keys are snake_case (e.g. <c>clipboard_sharing</c>) via the naming policy in
/// <see cref="Json.OmniJson"/>.
/// </summary>
/// <param name="Fingerprint">This machine's certificate fingerprint (what peers pin).</param>
/// <param name="Port">The UDP port the daemon listens on.</param>
/// <param name="Capturing">Whether local input capture is running (false = target-only).</param>
/// <param name="ClipboardSharing">Whether opt-in clipboard sharing is currently on.</param>
/// <param name="Sessions">Active sessions.</param>
/// <param name="Pending">Incoming requests awaiting accept/reject.</param>
public sealed record StatusInfo(
    string Fingerprint,
    int Port,
    bool Capturing,
    bool ClipboardSharing,
    IReadOnlyList<SessionInfo> Sessions,
    IReadOnlyList<PendingInfo> Pending);

/// <summary>One active session.</summary>
/// <param name="Host">The peer's host.</param>
/// <param name="Fingerprint">The peer's pinned certificate fingerprint.</param>
/// <param name="Role">This machine's role: "controller" or "target".</param>
/// <param name="Active">Whether input is currently routed to this peer.</param>
public sealed record SessionInfo(string Host, string Fingerprint, string Role, bool Active);

/// <summary>An incoming request awaiting accept/reject.</summary>
public sealed record PendingInfo(string Host, string Fingerprint);

/// <summary>One known peer. <c>Host</c> may be null if the peer was never named.</summary>
public sealed record PeerInfo(string? Host, string Fingerprint, bool Connected);

/// <summary>One peer's placement in the virtual desktop.</summary>
/// <param name="Host">The peer this placement is for.</param>
/// <param name="Edge">The edge this peer sits past: "left", "right", "top", or "bottom".</param>
/// <param name="Connected">True if from a live session; false if only saved for next time.</param>
public sealed record LayoutInfo(string Host, string Edge, bool Connected);
