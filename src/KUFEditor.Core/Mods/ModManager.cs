using System.IO.Compression;
using System.Text.Json;

namespace KUFEditor.Core.Mods;

/// <summary>
/// Manages mod installation, state persistence, and enumeration.
/// </summary>
public class ModManager
{
    private readonly string _modsDirectory;
    private readonly string _statePath;
    private Dictionary<string, List<string>> _state = new();
    private readonly List<Mod> _installedMods = new();

    public IReadOnlyList<Mod> InstalledMods => _installedMods;

    public ModManager(string modsDirectory)
    {
        _modsDirectory = modsDirectory;
        _statePath = Path.Combine(modsDirectory, "mods.json");

        if (!Directory.Exists(modsDirectory))
            Directory.CreateDirectory(modsDirectory);

        LoadState();
        ScanMods();
    }

    public void LoadState()
    {
        if (File.Exists(_statePath))
        {
            var json = File.ReadAllText(_statePath);
            _state = JsonSerializer.Deserialize<Dictionary<string, List<string>>>(json) ?? new();
        }
    }

    public void SaveState()
    {
        var dir = Path.GetDirectoryName(_statePath);
        if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
            Directory.CreateDirectory(dir);

        var json = JsonSerializer.Serialize(_state, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(_statePath, json);
    }

    public void ScanMods()
    {
        _installedMods.Clear();

        if (!Directory.Exists(_modsDirectory)) return;

        foreach (var file in Directory.GetFiles(_modsDirectory, "*.kufmod"))
        {
            var mod = LoadModFromZip(file);
            if (mod != null)
            {
                mod.SourcePath = file;
                _installedMods.Add(mod);
            }
        }
    }

    private Mod? LoadModFromZip(string path)
    {
        try
        {
            using var zip = ZipFile.OpenRead(path);
            var entry = zip.GetEntry("mod.json");
            if (entry == null) return null;

            using var stream = entry.Open();
            using var reader = new StreamReader(stream);
            var json = reader.ReadToEnd();

            var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
            return JsonSerializer.Deserialize<Mod>(json, options);
        }
        catch
        {
            return null;
        }
    }

    public List<string> GetEnabledMods(string game)
    {
        return _state.TryGetValue(game, out var list) ? new List<string>(list) : new List<string>();
    }

    public void SetEnabledMods(string game, List<string> modIds)
    {
        _state[game] = new List<string>(modIds);
        SaveState();
    }

    public bool IsEnabled(string game, string modId)
    {
        return GetEnabledMods(game).Contains(modId);
    }

    public void EnableMod(string game, string modId)
    {
        var enabled = GetEnabledMods(game);
        if (!enabled.Contains(modId))
        {
            enabled.Add(modId);
            SetEnabledMods(game, enabled);
        }
    }

    public void DisableMod(string game, string modId)
    {
        var enabled = GetEnabledMods(game);
        enabled.Remove(modId);
        SetEnabledMods(game, enabled);
    }

    public void ReorderMod(string game, string modId, int newIndex)
    {
        var enabled = GetEnabledMods(game);
        enabled.Remove(modId);
        enabled.Insert(Math.Min(newIndex, enabled.Count), modId);
        SetEnabledMods(game, enabled);
    }

    public void ImportMod(string sourcePath)
    {
        var fileName = Path.GetFileName(sourcePath);
        var destPath = Path.Combine(_modsDirectory, fileName);
        File.Copy(sourcePath, destPath, overwrite: true);
        ScanMods();
    }

    public void DeleteMod(Mod mod)
    {
        if (File.Exists(mod.SourcePath))
            File.Delete(mod.SourcePath);

        foreach (var game in _state.Keys.ToList())
            DisableMod(game, mod.Id);

        ScanMods();
    }

    public Mod? GetMod(string modId)
    {
        return _installedMods.FirstOrDefault(m => m.Id == modId);
    }

    public IEnumerable<Mod> GetModsForGame(string game)
    {
        return _installedMods.Where(m => m.Game == game);
    }
}
