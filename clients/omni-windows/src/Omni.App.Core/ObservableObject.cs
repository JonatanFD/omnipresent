using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Omni.App.Core;

/// <summary>
/// A tiny <see cref="INotifyPropertyChanged"/> base. Hand-rolled rather than
/// pulled from an MVVM library, to keep the client free of third-party UI
/// toolkits (constraint 15 in <c>docs/NATIVE_INTEGRATIONS.md</c>).
/// </summary>
public abstract class ObservableObject : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Sets a field and raises <see cref="PropertyChanged"/> if it changed.</summary>
    protected bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }
        field = value;
        Raise(propertyName);
        return true;
    }

    /// <summary>Raises <see cref="PropertyChanged"/> for the named property.</summary>
    protected void Raise([CallerMemberName] string? propertyName = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
}
