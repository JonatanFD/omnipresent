using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using Omni.App.Views;

namespace Omni.App;

/// <summary>
/// The application's main view: navigation container that switches between
/// sections (General, Connections, System, Update). Each section is a separate
/// UserControl hosted in the Frame.
/// </summary>
public sealed partial class MainView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public MainView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    protected override void OnLoaded()
    {
        base.OnLoaded();
        // Navigate to General by default
        NavView.SelectedItem = NavView.MenuItems[0];
    }

    private void OnNavItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        var tag = (args.InvokedItemContainer as NavigationViewItem)?.Tag as string;
        NavigateToSection(tag ?? "general");
    }

    private void NavigateToSection(string section)
    {
        var view = section switch
        {
            "general" => new GeneralView(ViewModel),
            "connections" => new ConnectionsView(ViewModel),
            "system" => new SystemView(ViewModel),
            "update" => new UpdateView(ViewModel),
            _ => new GeneralView(ViewModel),
        };

        ContentFrame.Content = view;
    }
}
