namespace Omni.App.Core;

/// <summary>
/// Marshals an action onto the UI thread. The view model runs its IPC work on
/// background tasks but must mutate bound state on the UI thread; this abstraction
/// lets the WinUI app use its <c>DispatcherQueue</c> while tests run synchronously.
/// </summary>
public interface IUiDispatcher
{
    void Post(Action action);
}

/// <summary>Runs the action inline. Used by tests (and any single-threaded caller).</summary>
public sealed class ImmediateDispatcher : IUiDispatcher
{
    public void Post(Action action) => action();
}
