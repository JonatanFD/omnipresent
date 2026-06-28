namespace Omni.App.Core;

/// <summary>How the app currently stands relative to the daemon.</summary>
public enum ConnectionStatus
{
    /// <summary>Trying to reach the daemon.</summary>
    Connecting,

    /// <summary>Live: receiving status snapshots.</summary>
    Connected,

    /// <summary>The daemon is not reachable; the app keeps retrying.</summary>
    Disconnected,

    /// <summary>The daemon speaks a newer protocol; the app must be updated.</summary>
    Incompatible,
}
