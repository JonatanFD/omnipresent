using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class PeerCard : UserControl
{
    public PeerInfo? Peer
    {
        get => (PeerInfo?)GetValue(PeerProperty);
        set => SetValue(PeerProperty, value);
    }

    public static readonly DependencyProperty PeerProperty =
        DependencyProperty.Register(
            nameof(Peer),
            typeof(PeerInfo),
            typeof(PeerCard),
            new PropertyMetadata(null));

    public string Host => Peer?.Host ?? "(unnamed)";
    public string Fingerprint => Peer?.Fingerprint ?? "";
    public string ButtonLabel { get; set; } = "Forget";

    public event RoutedEventHandler? ActionClicked;

    public PeerCard()
    {
        InitializeComponent();
    }

    private void OnButtonClick(object sender, RoutedEventArgs e)
    {
        ActionClicked?.Invoke(sender, e);
    }
}
