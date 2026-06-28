using System.Text.Json;
using System.Text.Json.Serialization;

namespace Omni.Ipc.Json;

/// <summary>
/// Reads and writes <see cref="OmniResponse"/> in the exact shape serde produces
/// for the Rust <c>Response</c> enum: an object tagged by a <c>"result"</c> field.
/// The <c>status</c> variant is a newtype that serde <i>flattens</i>, so the
/// <see cref="StatusInfo"/> fields sit next to the tag rather than under a nested
/// object — which is why a hand-written converter is needed instead of plain
/// polymorphism.
/// </summary>
public sealed class OmniResponseConverter : JsonConverter<OmniResponse>
{
    public override OmniResponse Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using var doc = JsonDocument.ParseValue(ref reader);
        var root = doc.RootElement;
        if (!root.TryGetProperty("result", out var tag) || tag.ValueKind != JsonValueKind.String)
        {
            throw new JsonException("response is missing its 'result' tag");
        }

        return tag.GetString() switch
        {
            "ok" => new OkResponse(),
            "error" => new ErrorResponse(GetString(root, "message")),
            "hello" => new HelloResponse(
                root.GetProperty("protocol_version").GetInt32(),
                GetString(root, "daemon_version")),
            "status" => new StatusResponse(root.Deserialize<StatusInfo>(options)!),
            "peers" => new PeersResponse(
                root.GetProperty("peers").Deserialize<List<PeerInfo>>(options) ?? []),
            "layout" => new LayoutResponse(
                root.GetProperty("placements").Deserialize<List<LayoutInfo>>(options) ?? []),
            var other => throw new JsonException($"unknown response result '{other}'"),
        };
    }

    public override void Write(Utf8JsonWriter writer, OmniResponse value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case OkResponse:
                writer.WriteString("result", "ok");
                break;
            case ErrorResponse e:
                writer.WriteString("result", "error");
                writer.WriteString("message", e.Message);
                break;
            case HelloResponse h:
                writer.WriteString("result", "hello");
                writer.WriteNumber("protocol_version", h.ProtocolVersion);
                writer.WriteString("daemon_version", h.DaemonVersion);
                break;
            case StatusResponse s:
                writer.WriteString("result", "status");
                JsonFlatten.WriteInto(writer, s.Status, options);
                break;
            case PeersResponse p:
                writer.WriteString("result", "peers");
                writer.WritePropertyName("peers");
                JsonSerializer.Serialize(writer, p.Peers, options);
                break;
            case LayoutResponse l:
                writer.WriteString("result", "layout");
                writer.WritePropertyName("placements");
                JsonSerializer.Serialize(writer, l.Placements, options);
                break;
            default:
                throw new JsonException($"cannot serialize response {value.GetType().Name}");
        }
        writer.WriteEndObject();
    }

    private static string GetString(JsonElement obj, string name) =>
        obj.TryGetProperty(name, out var value) ? value.GetString() ?? "" : "";
}
