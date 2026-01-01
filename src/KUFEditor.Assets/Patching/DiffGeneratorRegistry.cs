using System.Collections.Generic;
using System.IO;
using System.Linq;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Registry of all available diff generators.
/// </summary>
public class DiffGeneratorRegistry : IDiffGenerator
{
    private readonly List<IDiffGenerator> _generators = new();

    public DiffGeneratorRegistry()
    {
        _generators.Add(new TroopInfoDiffGenerator());
        _generators.Add(new TextSoxDiffGenerator());
    }

    public bool CanHandle(string fileName)
    {
        return _generators.Any(g => g.CanHandle(fileName));
    }

    public List<ModPatch> GenerateDiff(string originalPath, string modifiedPath, string relativePath)
    {
        var fileName = Path.GetFileName(modifiedPath);
        var generator = _generators.FirstOrDefault(g => g.CanHandle(fileName));

        if (generator == null)
            return new List<ModPatch>();

        return generator.GenerateDiff(originalPath, modifiedPath, relativePath);
    }

    /// <summary>
    /// Generates patches for all modified files in a directory.
    /// </summary>
    public List<ModPatch> GenerateDiffForDirectory(string originalDir, string modifiedDir)
    {
        var patches = new List<ModPatch>();

        if (!Directory.Exists(modifiedDir))
            return patches;

        foreach (var modifiedFile in Directory.GetFiles(modifiedDir, "*", SearchOption.AllDirectories))
        {
            var fileName = Path.GetFileName(modifiedFile);

            if (!CanHandle(fileName))
                continue;

            var relativePath = Path.GetRelativePath(modifiedDir, modifiedFile);
            var originalFile = Path.Combine(originalDir, relativePath);

            if (!File.Exists(originalFile))
                continue;

            var filPatches = GenerateDiff(originalFile, modifiedFile, relativePath);
            patches.AddRange(filPatches);
        }

        return patches;
    }
}
