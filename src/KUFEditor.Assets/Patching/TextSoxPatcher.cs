using System;
using System.Collections.Generic;
using System.Linq;
using KUFEditor.Assets.TextSox;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Patches text SOX files like ItemTypeInfo_ENG.sox.
/// </summary>
public class TextSoxPatcher : ISoxPatcher
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

    public void Modify(string filePath, string recordName, Dictionary<string, object> fields)
    {
        var data = TextSoxFile.Load(filePath);

        // recordName is the entry index (as string).
        if (!int.TryParse(recordName, out int index))
            throw new InvalidOperationException($"Invalid record name '{recordName}'. Expected an integer index.");

        var entry = data.Entries.FirstOrDefault(e => e.Index == index);
        if (entry == null)
            throw new InvalidOperationException($"Text entry at index {index} not found");

        if (fields.TryGetValue("Text", out var textValue))
        {
            var text = textValue?.ToString() ?? "";
            if (text.Length > entry.MaxLength)
                throw new InvalidOperationException($"Text exceeds max length of {entry.MaxLength}");
            entry.Text = text;
        }

        TextSoxFile.Save(data, filePath);
    }

    public void Add(string filePath, Dictionary<string, object> data)
    {
        // Text SOX files have fixed-length entries, adding new ones would break the structure.
        throw new NotSupportedException("Text SOX files have a fixed structure. Cannot add entries.");
    }

    public void Delete(string filePath, string recordName)
    {
        // Deleting would shift indices and break references.
        throw new NotSupportedException("Text SOX files have a fixed structure. Cannot delete entries.");
    }
}
