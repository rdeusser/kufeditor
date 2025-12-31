namespace KUFEditor.Core.KeyboardShortcuts;

/// <summary>
/// Represents a keyboard shortcut binding.
/// </summary>
public class ShortcutBinding
{
    private readonly Action _action;

    /// <summary>
    /// Gets the name of this shortcut.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// Gets the key combination for this shortcut.
    /// </summary>
    public ShortcutKey Key { get; }

    /// <summary>
    /// Gets or sets the display description for this shortcut.
    /// </summary>
    public string Description { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets whether this shortcut is enabled.
    /// </summary>
    public bool IsEnabled { get; set; } = true;

    /// <summary>
    /// Gets the category for grouping in UI.
    /// </summary>
    public string Category { get; }

    /// <summary>
    /// Creates a new shortcut binding.
    /// </summary>
    public ShortcutBinding(string name, ShortcutKey key, Action action, string category = "General")
    {
        Name = name ?? throw new ArgumentNullException(nameof(name));
        Key = key;
        _action = action ?? throw new ArgumentNullException(nameof(action));
        Category = category ?? "General";
    }

    /// <summary>
    /// Executes the shortcut action.
    /// </summary>
    public void Execute()
    {
        if (IsEnabled)
        {
            _action();
        }
    }
}
