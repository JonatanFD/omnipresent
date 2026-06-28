using Microsoft.UI.Xaml;
using Omni.App.Core;
using Windows.Graphics;

namespace Omni.App;

/// <summary>
/// The top-level window. It hosts <see cref="MainView"/> (which carries the UI and
/// its bindings) and sizes itself; the view does the rendering.
/// </summary>
public sealed partial class MainWindow : Window
{
    public MainWindow(DaemonViewModel viewModel)
    {
        InitializeComponent();
        Content = new MainView(viewModel);
        AppWindow.Resize(new SizeInt32(720, 900));
    }
}
