using System.Security.Cryptography;
using System.Text;

namespace Omni.Ipc;

/// <summary>
/// Locates the daemon's state directory and named pipe, reproducing the daemon's
/// own derivation (see <c>crates/omni-runtime/src/config.rs</c>) so a non-Rust
/// client can find it without the daemon publishing an address.
/// </summary>
public static class OmniPaths
{
    /// <summary>
    /// The daemon's state directory: <c>%OMNI_CONFIG_DIR%</c> if set, otherwise
    /// <c>%APPDATA%\omni</c> (the platform config dir, matching Rust's
    /// <c>dirs::config_dir()</c> on Windows).
    /// </summary>
    public static string ConfigDir()
    {
        var overridden = Environment.GetEnvironmentVariable("OMNI_CONFIG_DIR");
        if (!string.IsNullOrEmpty(overridden))
        {
            return overridden;
        }

        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "omni");
    }

    /// <summary>
    /// The short pipe name (without the <c>\\.\pipe\</c> prefix) for a state
    /// directory, as <see cref="System.IO.Pipes.NamedPipeClientStream"/> expects
    /// it. Derived from a stable SHA-256 of the directory path so two daemons with
    /// different <c>OMNI_CONFIG_DIR</c> get distinct pipes — and so this client and
    /// the Rust daemon agree on the name.
    /// </summary>
    public static string PipeShortName(string? configDir = null)
    {
        configDir ??= ConfigDir();
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(configDir));
        var hex = Convert.ToHexStringLower(digest.AsSpan(0, 8));
        return $"omni-{hex}";
    }

    /// <summary>The full kernel pipe path, for display and diagnostics.</summary>
    public static string PipeName(string? configDir = null) =>
        $@"\\.\pipe\{PipeShortName(configDir)}";
}
