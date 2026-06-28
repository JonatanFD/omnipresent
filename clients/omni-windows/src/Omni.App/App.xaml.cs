using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Omni.App.Core;
using Omni.Ipc;

namespace Omni.App;

/// <summary>
/// Application entry point. Wires the IPC client to the view model and the window,
/// and runs the view model's live loop for the app's lifetime. The Windows App SDK
/// generates the actual <c>Main</c> from this <see cref="Application"/> subclass.
/// </summary>
public partial class App : Application
{
    private readonly CancellationTokenSource _lifetime = new();
    private Window? _window;
    private DaemonViewModel? _viewModel;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var dispatcher = new WinUiDispatcher(DispatcherQueue.GetForCurrentThread());
        _viewModel = new DaemonViewModel(new OmniDaemonClient(), dispatcher);

        _window = new MainWindow(_viewModel);
        _window.Closed += (_, _) => _lifetime.Cancel();
        _window.Activate();

        // Follow the daemon for as long as the app is open; reconnects on its own.
        _ = _viewModel.RunAsync(_lifetime.Token);
    }
}
