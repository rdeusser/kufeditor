using System.Collections.Generic;

namespace KUFEditor.Core.Mods;

/// <summary>
/// Generates patches by comparing original and modified files.
/// </summary>
public interface IDiffGenerator
{
    /// <summary>
    /// Checks if this generator can handle the specified file.
    /// </summary>
    bool CanHandle(string fileName);

    /// <summary>
    /// Generates patches by comparing original and modified files.
    /// </summary>
    List<ModPatch> GenerateDiff(string originalPath, string modifiedPath, string relativePath);
}
