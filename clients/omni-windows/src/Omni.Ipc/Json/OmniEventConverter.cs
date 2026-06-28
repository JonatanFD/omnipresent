using System.Text.Json;
using System.Text.Json.Serialization;

namespace Omni.Ipc.Json;

/// <summary>
/// Reads and writes <see cref="OmniEvent"/> in the shape serde produces for the
/// Rust <c>Event</c> enum: an object tagged by an <c>"event"</c> field, with the
/// <see cref="StatusInfo"/> fields flattened next to the tag.
/// </summary>
public sealed class OmniEventConverter : JsonConverter<OmniEvent>
{
    public override OmniEvent Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        if (!root.TryGetProperty("event", out var tag) || tag.ValueKind != JsonValueKind.String)
        {
            throw new JsonException("event is missing its 'event' tag");
        }

        return tag.GetString() switch
        {
            "status" => new StatusEvent(root.Deserialize<StatusInfo>(options)!),
            var other => throw new JsonException($"unknown event '{other}'"),
        };
    }

    public override void Write(Utf8JsonWriter writer, OmniEvent value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case StatusEvent s:
                writer.WriteString("event", "status");
                JsonFlatten.WriteInto(writer, s.Status, options);
                break;
            default:
                throw new JsonException($"cannot serialize event {value.GetType().Name}");
        }
        writer.WriteEndObject();
    }
}
