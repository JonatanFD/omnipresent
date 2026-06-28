using Omni.Ipc;

namespace Omni.Ipc.Tests;

/// <summary>
/// The pipe name must match the daemon's derivation exactly, or the client cannot
/// find the daemon. The Rust side asserts the same known vector
/// (<c>crates/omni-runtime/src/config.rs</c>), so the two stay in lock-step.
/// </summary>
public class OmniPathsTests
{
    [Fact]
    public void Pipe_name_matches_the_shared_known_vector()
    {
        // sha256("C:\\example\\omni")[..8] in lowercase hex. The identical vector
        // is asserted by the Rust `pipe_name_is_a_stable_hash` test.
        Assert.Equal("omni-3bf61631564ef580", OmniPaths.PipeShortName(@"C:\example\omni"));
        Assert.Equal(@"\\.\pipe\omni-3bf61631564ef580", OmniPaths.PipeName(@"C:\example\omni"));
    }

    [Fact]
    public void Pipe_name_is_deterministic_and_dir_specific()
    {
        Assert.Equal(OmniPaths.PipeShortName(@"C:\a"), OmniPaths.PipeShortName(@"C:\a"));
        Assert.NotEqual(OmniPaths.PipeShortName(@"C:\a"), OmniPaths.PipeShortName(@"C:\b"));
    }

    [Fact]
    public void Config_dir_honors_the_override_env_var()
    {
        var previous = Environment.GetEnvironmentVariable("OMNI_CONFIG_DIR");
        try
        {
            Environment.SetEnvironmentVariable("OMNI_CONFIG_DIR", @"C:\custom\state");
            Assert.Equal(@"C:\custom\state", OmniPaths.ConfigDir());
            Assert.Equal(OmniPaths.PipeShortName(@"C:\custom\state"), OmniPaths.PipeShortName());
        }
        finally
        {
            Environment.SetEnvironmentVariable("OMNI_CONFIG_DIR", previous);
        }
    }
}
