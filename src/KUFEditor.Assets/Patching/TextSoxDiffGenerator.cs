using System;
using System.Collections.Generic;
using System.Linq;
using KUFEditor.Assets.TextSox;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Generates diff patches for text SOX files.
/// </summary>
public class TextSoxDiffGenerator : IDiffGenerator
{
    private static readonly string[] SupportedFiles = new[]
    {
        "ItemTypeInfo_ENG.sox",
        "ItemTypeInfo_KOR.sox",
        "ItemTypeInfo_JPN.sox"
    };

    public bool CanHandle(string fileName)
    {
        return SupportedFiles.Any(f => f.Equals(fileName, StringComparison.OrdinalIgnoreCase));
    }

    public List<ModPatch> GenerateDiff(string originalPath, string modifiedPath, string relativePath)
    {
        var patches = new List<ModPatch>();

        var original = TextSoxFile.Load(originalPath);
        var modified = TextSoxFile.Load(modifiedPath);

        // Compare each entry.
        for (int i = 0; i < Math.Min(original.Entries.Count, modified.Entries.Count); i++)
        {
            var origEntry = original.Entries[i];
            var modEntry = modified.Entries[i];

            if (origEntry.Text != modEntry.Text)
            {
                patches.Add(new ModPatch
                {
                    File = relativePath,
                    Action = PatchAction.Modify,
                    Record = i.ToString(),
                    Fields = new Dictionary<string, object>
                    {
                        { "Text", modEntry.Text }
                    }
                });
            }
        }

        return patches;
    }
}
