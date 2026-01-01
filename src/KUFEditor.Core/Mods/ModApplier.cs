namespace KUFEditor.Core.Mods;

/// <summary>
/// Applies mods to game files with conflict detection.
/// </summary>
public class ModApplier
{
    private readonly string _gameDirectory;
    private readonly BackupManager? _backupManager;
    private readonly Dictionary<string, (string modId, object? value)> _touchedFields = new();

    public List<ModConflict> Conflicts { get; } = new();

    public ModApplier(string gameDirectory, BackupManager? backupManager = null)
    {
        _gameDirectory = gameDirectory;
        _backupManager = backupManager;
    }

    /// <summary>
    /// Detects conflicts between mods without applying them.
    /// </summary>
    public void DetectConflicts(List<Mod> mods)
    {
        _touchedFields.Clear();
        Conflicts.Clear();

        foreach (var mod in mods)
        {
            foreach (var patch in mod.Patches)
            {
                TrackPatch(mod.Id, patch);
            }
        }
    }

    /// <summary>
    /// Applies mods to game files, restoring from pristine first.
    /// </summary>
    public void Apply(List<Mod> mods, string gameName)
    {
        _touchedFields.Clear();
        Conflicts.Clear();

        // Restore from pristine first.
        RestoreFromPristine(mods, gameName);

        // Apply each mod in order.
        foreach (var mod in mods)
        {
            ApplyMod(mod);
        }
    }

    private void RestoreFromPristine(List<Mod> mods, string gameName)
    {
        if (_backupManager == null) return;

        var filesToRestore = mods
            .SelectMany(m => m.Patches)
            .Select(p => p.File)
            .Distinct();

        foreach (var file in filesToRestore)
        {
            var destPath = Path.Combine(_gameDirectory, file);
            try
            {
                _backupManager.RestoreFromPristine(gameName, file, destPath);
            }
            catch
            {
                // File may not have a pristine backup yet.
            }
        }
    }

    private void ApplyMod(Mod mod)
    {
        foreach (var patch in mod.Patches)
        {
            TrackPatch(mod.Id, patch);
            ApplyPatch(patch);
        }
    }

    private void TrackPatch(string modId, ModPatch patch)
    {
        if (patch.Action == PatchAction.Modify && patch.Fields != null)
        {
            foreach (var field in patch.Fields)
            {
                var key = $"{patch.File}|{patch.Record}|{field.Key}";

                if (_touchedFields.TryGetValue(key, out var prev))
                {
                    Conflicts.Add(new ModConflict
                    {
                        File = patch.File,
                        Record = patch.Record ?? "",
                        Field = field.Key,
                        FirstModId = prev.modId,
                        FirstValue = prev.value,
                        SecondModId = modId,
                        SecondValue = field.Value
                    });
                }

                _touchedFields[key] = (modId, field.Value);
            }
        }
    }

    private void ApplyPatch(ModPatch patch)
    {
        // Actual file patching will be implemented when we integrate with SOX parsers.
        // For now, conflict detection is the primary function.
        var filePath = Path.Combine(_gameDirectory, patch.File);

        switch (patch.Action)
        {
            case PatchAction.Modify:
                // TODO: Load SOX file, find record by name, modify fields, save.
                break;
            case PatchAction.Add:
                // TODO: Load SOX file, add new record with data, save.
                break;
            case PatchAction.Delete:
                // TODO: Load SOX file, remove record by name, save.
                break;
        }
    }

    /// <summary>
    /// Gets a summary of files that will be modified by the given mods.
    /// </summary>
    public Dictionary<string, int> GetAffectedFiles(List<Mod> mods)
    {
        return mods
            .SelectMany(m => m.Patches)
            .GroupBy(p => p.File)
            .ToDictionary(g => g.Key, g => g.Count());
    }
}
