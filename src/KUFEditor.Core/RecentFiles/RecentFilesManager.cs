using System.Text.Json;

namespace KUFEditor.Core.RecentFiles;

/// <summary>
/// Manages a list of recently opened files with persistence.
/// </summary>
public class RecentFilesManager
{
    private readonly List<RecentFile> _files = new();
    private readonly int _maxFiles;
    private string? _persistPath;

    /// <summary>
    /// Raised when the recent files list changes.
    /// </summary>
    public event EventHandler? Changed;

    /// <summary>
    /// Gets the maximum number of files to track.
    /// </summary>
    public int MaxFiles => _maxFiles;

    /// <summary>
    /// Gets the current list of recent files.
    /// </summary>
    public IReadOnlyList<RecentFile> Files => _files;

    /// <summary>
    /// Gets the number of recent files.
    /// </summary>
    public int Count => _files.Count;

    /// <summary>
    /// Creates a new recent files manager.
    /// </summary>
    public RecentFilesManager(int maxFiles = 10)
    {
        if (maxFiles < 1)
            throw new ArgumentOutOfRangeException(nameof(maxFiles), "Max files must be at least 1");

        _maxFiles = maxFiles;
    }

    /// <summary>
    /// Adds a file to the recent list.
    /// </summary>
    public void Add(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        // Remove if already exists to move it to the top.
        _files.RemoveAll(f => f.Path.Equals(path, StringComparison.OrdinalIgnoreCase));

        // Add at the beginning.
        _files.Insert(0, new RecentFile(path, DateTime.UtcNow));

        // Trim to max size.
        while (_files.Count > _maxFiles)
        {
            _files.RemoveAt(_files.Count - 1);
        }

        OnChanged();
    }

    /// <summary>
    /// Removes a file from the recent list.
    /// </summary>
    public bool Remove(string path)
    {
        var removed = _files.RemoveAll(f => f.Path.Equals(path, StringComparison.OrdinalIgnoreCase)) > 0;
        if (removed)
        {
            OnChanged();
        }
        return removed;
    }

    /// <summary>
    /// Clears all recent files.
    /// </summary>
    public void Clear()
    {
        if (_files.Count > 0)
        {
            _files.Clear();
            OnChanged();
        }
    }

    /// <summary>
    /// Checks if a file is in the recent list.
    /// </summary>
    public bool Contains(string path)
    {
        return _files.Any(f => f.Path.Equals(path, StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>
    /// Gets the most recent file, or null if none.
    /// </summary>
    public RecentFile? GetMostRecent()
    {
        return _files.Count > 0 ? _files[0] : null;
    }

    /// <summary>
    /// Removes files that no longer exist on disk.
    /// </summary>
    public int RemoveNonExistent()
    {
        var removed = _files.RemoveAll(f => !File.Exists(f.Path));
        if (removed > 0)
        {
            OnChanged();
        }
        return removed;
    }

    /// <summary>
    /// Saves the recent files list to a file.
    /// </summary>
    public void Save(string path)
    {
        _persistPath = path;

        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
        {
            Directory.CreateDirectory(dir);
        }

        var data = new RecentFilesData { Files = _files.ToList() };
        var json = JsonSerializer.Serialize(data, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(path, json);
    }

    /// <summary>
    /// Loads the recent files list from a file.
    /// </summary>
    public void Load(string path)
    {
        _persistPath = path;

        if (!File.Exists(path))
        {
            return;
        }

        try
        {
            var json = File.ReadAllText(path);
            var data = JsonSerializer.Deserialize<RecentFilesData>(json);

            if (data?.Files != null)
            {
                _files.Clear();
                foreach (var file in data.Files.Take(_maxFiles))
                {
                    _files.Add(file);
                }
            }
        }
        catch (JsonException)
        {
            // ignore corrupt files
        }
    }

    /// <summary>
    /// Gets the default path for storing recent files.
    /// </summary>
    public static string GetDefaultPath()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "KUFEditor", "recent_files.json");
    }

    private void OnChanged()
    {
        Changed?.Invoke(this, EventArgs.Empty);

        // Auto-save if a persist path is set.
        if (!string.IsNullOrEmpty(_persistPath))
        {
            Save(_persistPath);
        }
    }

    private class RecentFilesData
    {
        public List<RecentFile> Files { get; set; } = new();
    }
}

/// <summary>
/// Represents a recently opened file.
/// </summary>
public class RecentFile
{
    /// <summary>
    /// Gets the file path.
    /// </summary>
    public string Path { get; set; } = string.Empty;

    /// <summary>
    /// Gets the time the file was last opened.
    /// </summary>
    public DateTime LastOpened { get; set; }

    /// <summary>
    /// Gets the file name.
    /// </summary>
    public string FileName => System.IO.Path.GetFileName(Path);

    /// <summary>
    /// Gets the directory containing the file.
    /// </summary>
    public string Directory => System.IO.Path.GetDirectoryName(Path) ?? string.Empty;

    public RecentFile()
    {
    }

    public RecentFile(string path, DateTime lastOpened)
    {
        Path = path;
        LastOpened = lastOpened;
    }
}
