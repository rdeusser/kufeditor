using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Registry of all available SOX patchers.
/// </summary>
public class SoxPatcherRegistry : ISoxPatcher
{
    private readonly List<ISoxPatcher> _patchers = new();

    public SoxPatcherRegistry()
    {
        _patchers.Add(new TroopInfoPatcher());
        _patchers.Add(new TextSoxPatcher());
    }

    public bool CanHandle(string fileName)
    {
        return _patchers.Any(p => p.CanHandle(fileName));
    }

    public void Modify(string filePath, string recordName, Dictionary<string, object> fields)
    {
        var fileName = Path.GetFileName(filePath);
        var patcher = GetPatcher(fileName);
        patcher.Modify(filePath, recordName, fields);
    }

    public void Add(string filePath, Dictionary<string, object> data)
    {
        var fileName = Path.GetFileName(filePath);
        var patcher = GetPatcher(fileName);
        patcher.Add(filePath, data);
    }

    public void Delete(string filePath, string recordName)
    {
        var fileName = Path.GetFileName(filePath);
        var patcher = GetPatcher(fileName);
        patcher.Delete(filePath, recordName);
    }

    private ISoxPatcher GetPatcher(string fileName)
    {
        var patcher = _patchers.FirstOrDefault(p => p.CanHandle(fileName));
        if (patcher == null)
            throw new NotSupportedException($"No patcher available for '{fileName}'");
        return patcher;
    }
}
