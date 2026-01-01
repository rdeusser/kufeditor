using System.Text.Json.Serialization;

namespace KUFEditor.Core.Mods;

/// <summary>
/// Specifies the type of patch operation to perform on a game file.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter))]
public enum PatchAction
{
    Modify,
    Add,
    Delete
}

/// <summary>
/// Represents a single patch operation within a mod.
/// </summary>
public class ModPatch
{
    public string File { get; set; } = string.Empty;
    public PatchAction Action { get; set; }
    public string? Record { get; set; }
    public Dictionary<string, object>? Fields { get; set; }
    public Dictionary<string, object>? Data { get; set; }
}

/// <summary>
/// Represents a mod package containing metadata and patches to apply to game files.
/// </summary>
public class Mod
{
    public string Id { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string Version { get; set; } = "1.0.0";
    public string Author { get; set; } = string.Empty;
    public string Description { get; set; } = string.Empty;
    public string Game { get; set; } = string.Empty;
    public List<ModPatch> Patches { get; set; } = new();

    [JsonIgnore]
    public string SourcePath { get; set; } = string.Empty;
}
