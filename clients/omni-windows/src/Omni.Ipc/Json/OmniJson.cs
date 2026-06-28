using System.Text.Json;

namespace Omni.Ipc.Json;

/// <summary>
/// The single <see cref="JsonSerializerOptions"/> used for every IPC message, so
/// the wire format matches the daemon's serde output exactly: snake_case property
/// names (e.g. <c>clipboard_sharing</c>) and the tagged-union converters.
/// </summary>
public static class OmniJson
{
    /// <summary>Shared, immutable options. Safe to reuse across threads.</summary>
    public static JsonSerializerOptions Options { get; } = Create();

    private static JsonSerializerOptions Create()
    {
        var options = new JsonSerializerOptions
        {
            // serde leaves struct fields in snake_case; mirror that here.
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
            PropertyNameCaseInsensitive = false,
            // One message per line: never emit indentation.
            WriteIndented = false,
        };
        // OmniResponse / OmniEvent carry [JsonConverter] attributes, so they are
        // picked up automatically; nothing else to register here.
        return options;
    }
}
