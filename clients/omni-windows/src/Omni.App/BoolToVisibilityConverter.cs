using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;

namespace Omni.App;

/// <summary>
/// Maps a bool to <see cref="Visibility"/> for x:Bind. Pass <c>Invert</c> as the
/// converter parameter to collapse when true. WinUI 3 ships no built-in
/// equivalent, and this is a non-visual helper (not a third-party UI toolkit).
/// </summary>
public sealed class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var flag = value is true;
        if (string.Equals(parameter as string, "Invert", StringComparison.OrdinalIgnoreCase))
        {
            flag = !flag;
        }
        return flag ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language) =>
        throw new NotSupportedException();
}
