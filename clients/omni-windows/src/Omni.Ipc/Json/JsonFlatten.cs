using System.Text.Json;

namespace Omni.Ipc.Json;

/// <summary>
/// Helper for serde-style "flattened" newtype variants: writes an object's
/// properties directly into the current object, next to a tag, instead of nesting
/// them under a property.
/// </summary>
internal static class JsonFlatten
{
    /// <summary>
    /// Serializes <paramref name="value"/> and writes each of its properties into
    /// the object <paramref name="writer"/> is currently building.
    /// </summary>
    public static void WriteInto<T>(Utf8JsonWriter writer, T value, JsonSerializerOptions options)
    {
        var element = JsonSerializer.SerializeToElement(value, options);
        foreach (var property in element.EnumerateObject())
        {
            property.WriteTo(writer);
        }
    }
}
