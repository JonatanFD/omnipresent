using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class PendingRequestCard : UserControl
{
    public PendingInfo? Request
    {
        get => (PendingInfo?)GetValue(RequestProperty);
        set => SetValue(RequestProperty, value);
    }

    public static readonly DependencyProperty RequestProperty =
        DependencyProperty.Register(
            nameof(Request),
            typeof(PendingInfo),
            typeof(PendingRequestCard),
            new PropertyMetadata(null));

    public string Host => Request?.Host ?? "";
    public string Fingerprint => Request?.Fingerprint ?? "";

    public event RoutedEventHandler? AcceptClicked;
    public event RoutedEventHandler? RejectClicked;

    public PendingRequestCard()
    {
        InitializeComponent();
    }

    private void OnAcceptClick(object sender, RoutedEventArgs e)
    {
        AcceptClicked?.Invoke(sender, e);
    }

    private void OnRejectClick(object sender, RoutedEventArgs e)
    {
        RejectClicked?.Invoke(sender, e);
    }
}
