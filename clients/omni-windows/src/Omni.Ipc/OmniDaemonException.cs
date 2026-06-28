namespace Omni.Ipc;

/// <summary>
/// Thrown when a command to the daemon fails: the daemon is not reachable, the
/// connection broke, or the daemon answered with an <see cref="ErrorResponse"/>.
/// </summary>
public sealed class OmniDaemonException : Exception
{
    public OmniDaemonException(string message) : base(message)
    {
    }

    public OmniDaemonException(string message, Exception inner) : base(message, inner)
    {
    }
}
