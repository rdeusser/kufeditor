namespace KUFEditor.Core.Files;

/// <summary>
/// Manages open files and their state.
/// </summary>
public class FileManager
{
    private readonly Dictionary<string, OpenFile> _files = new(StringComparer.OrdinalIgnoreCase);
    private OpenFile? _activeFile;

    /// <summary>
    /// Raised when a file is opened.
    /// </summary>
    public event EventHandler<OpenFile>? FileOpened;

    /// <summary>
    /// Raised when a file is closed.
    /// </summary>
    public event EventHandler<OpenFile>? FileClosed;

    /// <summary>
    /// Raised when the active file changes.
    /// </summary>
    public event EventHandler<OpenFile?>? ActiveFileChanged;

    /// <summary>
    /// Raised when any file's dirty state changes.
    /// </summary>
    public event EventHandler<OpenFile>? FileDirtyChanged;

    /// <summary>
    /// Raised when a file is saved.
    /// </summary>
    public event EventHandler<OpenFile>? FileSaved;

    /// <summary>
    /// Gets all open files.
    /// </summary>
    public IReadOnlyCollection<OpenFile> Files => _files.Values;

    /// <summary>
    /// Gets the number of open files.
    /// </summary>
    public int Count => _files.Count;

    /// <summary>
    /// Gets the active file.
    /// </summary>
    public OpenFile? ActiveFile => _activeFile;

    /// <summary>
    /// Gets whether any file has unsaved changes.
    /// </summary>
    public bool HasUnsavedChanges => _files.Values.Any(f => f.IsDirty);

    /// <summary>
    /// Gets files with unsaved changes.
    /// </summary>
    public IEnumerable<OpenFile> DirtyFiles => _files.Values.Where(f => f.IsDirty);

    /// <summary>
    /// Opens a file.
    /// </summary>
    public OpenFile Open(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (_files.TryGetValue(path, out var existing))
        {
            SetActive(existing);
            return existing;
        }

        var file = new OpenFile(path);
        file.DirtyChanged += OnFileDirtyChanged;
        _files[path] = file;

        FileOpened?.Invoke(this, file);
        SetActive(file);

        return file;
    }

    /// <summary>
    /// Closes a file.
    /// </summary>
    public bool Close(string path)
    {
        if (!_files.TryGetValue(path, out var file))
            return false;

        file.DirtyChanged -= OnFileDirtyChanged;
        _files.Remove(path);

        FileClosed?.Invoke(this, file);

        if (_activeFile == file)
        {
            SetActive(_files.Values.FirstOrDefault());
        }

        return true;
    }

    /// <summary>
    /// Closes a file, returns false if dirty and not forced.
    /// </summary>
    public bool Close(OpenFile file, bool force = false)
    {
        if (file.IsDirty && !force)
            return false;

        return Close(file.Path);
    }

    /// <summary>
    /// Gets a file by path.
    /// </summary>
    public OpenFile? Get(string path)
    {
        return _files.TryGetValue(path, out var file) ? file : null;
    }

    /// <summary>
    /// Checks if a file is open.
    /// </summary>
    public bool IsOpen(string path)
    {
        return _files.ContainsKey(path);
    }

    /// <summary>
    /// Sets the active file.
    /// </summary>
    public void SetActive(OpenFile? file)
    {
        if (_activeFile == file)
            return;

        _activeFile = file;
        ActiveFileChanged?.Invoke(this, file);
    }

    /// <summary>
    /// Sets the active file by path.
    /// </summary>
    public bool SetActive(string path)
    {
        if (_files.TryGetValue(path, out var file))
        {
            SetActive(file);
            return true;
        }
        return false;
    }

    /// <summary>
    /// Saves the active file.
    /// </summary>
    public bool Save()
    {
        if (_activeFile == null)
            return false;

        return Save(_activeFile);
    }

    /// <summary>
    /// Saves a specific file.
    /// </summary>
    public bool Save(OpenFile file)
    {
        if (!file.IsDirty)
            return true;

        file.Save();
        FileSaved?.Invoke(this, file);
        return true;
    }

    /// <summary>
    /// Saves all dirty files.
    /// </summary>
    public int SaveAll()
    {
        var saved = 0;
        foreach (var file in _files.Values.Where(f => f.IsDirty).ToList())
        {
            if (Save(file))
                saved++;
        }
        return saved;
    }

    /// <summary>
    /// Closes all files.
    /// </summary>
    public bool CloseAll(bool force = false)
    {
        if (!force && HasUnsavedChanges)
            return false;

        var paths = _files.Keys.ToList();
        foreach (var path in paths)
        {
            Close(path);
        }
        return true;
    }

    /// <summary>
    /// Switches to the next file.
    /// </summary>
    public OpenFile? NextFile()
    {
        if (_files.Count <= 1)
            return _activeFile;

        var files = _files.Values.ToList();
        var index = _activeFile != null ? files.IndexOf(_activeFile) : -1;
        var next = (index + 1) % files.Count;

        SetActive(files[next]);
        return _activeFile;
    }

    /// <summary>
    /// Switches to the previous file.
    /// </summary>
    public OpenFile? PreviousFile()
    {
        if (_files.Count <= 1)
            return _activeFile;

        var files = _files.Values.ToList();
        var index = _activeFile != null ? files.IndexOf(_activeFile) : 0;
        var prev = (index - 1 + files.Count) % files.Count;

        SetActive(files[prev]);
        return _activeFile;
    }

    private void OnFileDirtyChanged(object? sender, EventArgs e)
    {
        if (sender is OpenFile file)
        {
            FileDirtyChanged?.Invoke(this, file);
        }
    }
}
