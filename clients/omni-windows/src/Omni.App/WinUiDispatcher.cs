using Microsoft.UI.Dispatching;
using Omni.App.Core;

namespace Omni.App;

/// <summary>
/// Marshals view-model updates onto the WinUI UI thread via the window's
/// <see cref="DispatcherQueue"/>.
/// </summary>
public sealed class WinUiDispatcher : IUiDispatcher
{
    private readonly DispatcherQueue _queue;

    public WinUiDispatcher(DispatcherQueue queue) => _queue = queue;

    public void Post(Action action)
    {
        if (!_queue.TryEnqueue(() => action()))
        {
            // The queue is shutting down; run inline as a last resort.
            action();
        }
    }
}
