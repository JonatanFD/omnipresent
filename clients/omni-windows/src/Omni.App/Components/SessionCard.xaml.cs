using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class SessionCard : UserControl
{
    public SessionInfo? Session
    {
        get => (SessionInfo?)GetValue(SessionProperty);
        set => SetValue(SessionProperty, value);
    }

    public static readonly DependencyProperty SessionProperty =
        DependencyProperty.Register(
            nameof(Session),
            typeof(SessionInfo),
            typeof(SessionCard),
            new PropertyMetadata(null));

    public string Host => Session?.Host ?? "";
    public string Role => Session?.Role ?? "";
    public string ButtonLabel { get; set; } = "Disconnect";

    public event RoutedEventHandler? ActionClicked;

    public SessionCard()
    {
        InitializeComponent();
    }

    private void OnButtonClick(object sender, RoutedEventArgs e)
    {
        ActionClicked?.Invoke(sender, e);
    }
}
