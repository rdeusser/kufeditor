namespace KUFEditor.Core.KeyboardShortcuts;

/// <summary>
/// Manages keyboard shortcut bindings and execution.
/// </summary>
public class ShortcutManager
{
    private readonly Dictionary<ShortcutKey, ShortcutBinding> _bindings = new();
    private readonly Dictionary<string, ShortcutBinding> _bindingsByName = new();

    /// <summary>
    /// Raised when a shortcut is executed.
    /// </summary>
    public event EventHandler<ShortcutExecutedEventArgs>? ShortcutExecuted;

    /// <summary>
    /// Gets all registered bindings.
    /// </summary>
    public IReadOnlyCollection<ShortcutBinding> Bindings => _bindings.Values;

    /// <summary>
    /// Registers a keyboard shortcut.
    /// </summary>
    public void Register(ShortcutBinding binding)
    {
        ArgumentNullException.ThrowIfNull(binding);

        _bindings[binding.Key] = binding;
        _bindingsByName[binding.Name] = binding;
    }

    /// <summary>
    /// Unregisters a keyboard shortcut by key.
    /// </summary>
    public bool Unregister(ShortcutKey key)
    {
        if (_bindings.TryGetValue(key, out var binding))
        {
            _bindings.Remove(key);
            _bindingsByName.Remove(binding.Name);
            return true;
        }
        return false;
    }

    /// <summary>
    /// Unregisters a keyboard shortcut by name.
    /// </summary>
    public bool Unregister(string name)
    {
        if (_bindingsByName.TryGetValue(name, out var binding))
        {
            _bindingsByName.Remove(name);
            _bindings.Remove(binding.Key);
            return true;
        }
        return false;
    }

    /// <summary>
    /// Gets a binding by name.
    /// </summary>
    public ShortcutBinding? GetBinding(string name)
    {
        return _bindingsByName.TryGetValue(name, out var binding) ? binding : null;
    }

    /// <summary>
    /// Tries to handle a key event and execute the matching shortcut.
    /// </summary>
    /// <returns>True if a shortcut was executed, false otherwise.</returns>
    public bool TryHandle(ShortcutKey key)
    {
        if (_bindings.TryGetValue(key, out var binding))
        {
            if (binding.IsEnabled)
            {
                binding.Execute();
                ShortcutExecuted?.Invoke(this, new ShortcutExecutedEventArgs(binding));
                return true;
            }
        }
        return false;
    }

    /// <summary>
    /// Clears all registered bindings.
    /// </summary>
    public void Clear()
    {
        _bindings.Clear();
        _bindingsByName.Clear();
    }

    /// <summary>
    /// Gets the shortcut display text for a binding.
    /// </summary>
    public static string GetDisplayText(ShortcutKey key)
    {
        var parts = new List<string>();

        if (key.HasModifier(KeyModifiers.Control))
            parts.Add(OperatingSystem.IsMacOS() ? "⌘" : "Ctrl");
        if (key.HasModifier(KeyModifiers.Alt))
            parts.Add(OperatingSystem.IsMacOS() ? "⌥" : "Alt");
        if (key.HasModifier(KeyModifiers.Shift))
            parts.Add(OperatingSystem.IsMacOS() ? "⇧" : "Shift");

        parts.Add(key.Key.ToString());
        return string.Join(OperatingSystem.IsMacOS() ? "" : "+", parts);
    }
}

/// <summary>
/// Event args for when a shortcut is executed.
/// </summary>
public class ShortcutExecutedEventArgs : EventArgs
{
    public ShortcutBinding Binding { get; }

    public ShortcutExecutedEventArgs(ShortcutBinding binding)
    {
        Binding = binding;
    }
}
