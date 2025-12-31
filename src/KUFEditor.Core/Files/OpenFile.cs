namespace KUFEditor.Core.Files;

/// <summary>
/// Represents a file open in the editor.
/// </summary>
public class OpenFile
{
    private bool _isDirty;
    private object? _data;
    private Action<OpenFile>? _saveHandler;

    /// <summary>
    /// Raised when the dirty state changes.
    /// </summary>
    public event EventHandler? DirtyChanged;

    /// <summary>
    /// Raised when the file is saved.
    /// </summary>
    public event EventHandler? Saved;

    /// <summary>
    /// Gets the file path.
    /// </summary>
    public string Path { get; }

    /// <summary>
    /// Gets the file name.
    /// </summary>
    public string Name => System.IO.Path.GetFileName(Path);

    /// <summary>
    /// Gets the file extension.
    /// </summary>
    public string Extension => System.IO.Path.GetExtension(Path);

    /// <summary>
    /// Gets the directory containing the file.
    /// </summary>
    public string Directory => System.IO.Path.GetDirectoryName(Path) ?? string.Empty;

    /// <summary>
    /// Gets or sets whether the file has unsaved changes.
    /// </summary>
    public bool IsDirty
    {
        get => _isDirty;
        set
        {
            if (_isDirty != value)
            {
                _isDirty = value;
                DirtyChanged?.Invoke(this, EventArgs.Empty);
            }
        }
    }

    /// <summary>
    /// Gets or sets the file data.
    /// </summary>
    public object? Data
    {
        get => _data;
        set
        {
            _data = value;
            IsDirty = true;
        }
    }

    /// <summary>
    /// Gets or sets the file type.
    /// </summary>
    public FileType Type { get; set; }

    /// <summary>
    /// Gets the display title (includes * for dirty).
    /// </summary>
    public string Title => IsDirty ? $"{Name}*" : Name;

    /// <summary>
    /// Gets the time the file was opened.
    /// </summary>
    public DateTime OpenedAt { get; }

    /// <summary>
    /// Gets the last save time, or null if never saved.
    /// </summary>
    public DateTime? LastSavedAt { get; private set; }

    /// <summary>
    /// Creates a new open file.
    /// </summary>
    public OpenFile(string path)
    {
        Path = path ?? throw new ArgumentNullException(nameof(path));
        Type = DetermineType(path);
        OpenedAt = DateTime.UtcNow;
    }

    /// <summary>
    /// Sets the save handler for this file.
    /// </summary>
    public void SetSaveHandler(Action<OpenFile> handler)
    {
        _saveHandler = handler;
    }

    /// <summary>
    /// Saves the file.
    /// </summary>
    public void Save()
    {
        _saveHandler?.Invoke(this);
        IsDirty = false;
        LastSavedAt = DateTime.UtcNow;
        Saved?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>
    /// Marks the file as modified.
    /// </summary>
    public void MarkDirty()
    {
        IsDirty = true;
    }

    /// <summary>
    /// Marks the file as clean (saved).
    /// </summary>
    public void MarkClean()
    {
        IsDirty = false;
    }

    private static FileType DetermineType(string path)
    {
        var ext = System.IO.Path.GetExtension(path).ToLowerInvariant();
        var name = System.IO.Path.GetFileName(path).ToLowerInvariant();

        return ext switch
        {
            ".sox" when name.Contains("troopinfo") => FileType.TroopInfoSox,
            ".sox" when name.Contains("skillinfo") => FileType.SkillInfoSox,
            ".sox" when name.Contains("expinfo") => FileType.ExpInfoSox,
            ".sox" when name.Contains("itemtypeinfo") => FileType.TextSox,
            ".sox" when name.Contains("_eng") || name.Contains("_kor") => FileType.TextSox,
            ".sox" => FileType.BinarySox,
            ".stg" => FileType.Mission,
            ".sav" => FileType.SaveGame,
            ".nav" => FileType.Navigation,
            ".k2a" => FileType.Model,
            ".txt" => FileType.Text,
            ".xml" => FileType.Xml,
            _ => FileType.Unknown
        };
    }
}

/// <summary>
/// Types of files the editor can handle.
/// </summary>
public enum FileType
{
    Unknown,
    TroopInfoSox,
    SkillInfoSox,
    ExpInfoSox,
    TextSox,
    BinarySox,
    Mission,
    SaveGame,
    Navigation,
    Model,
    Text,
    Xml
}
