namespace KUFEditor.Core.KeyboardShortcuts;

/// <summary>
/// Represents a key combination for keyboard shortcuts.
/// </summary>
public readonly struct ShortcutKey : IEquatable<ShortcutKey>
{
    /// <summary>
    /// Gets the primary key code.
    /// </summary>
    public KeyCode Key { get; }

    /// <summary>
    /// Gets the modifier keys.
    /// </summary>
    public KeyModifiers Modifiers { get; }

    /// <summary>
    /// Creates a new shortcut key.
    /// </summary>
    public ShortcutKey(KeyCode key, KeyModifiers modifiers = KeyModifiers.None)
    {
        Key = key;
        Modifiers = modifiers;
    }

    /// <summary>
    /// Checks if this key has the specified modifier.
    /// </summary>
    public bool HasModifier(KeyModifiers modifier)
    {
        return (Modifiers & modifier) == modifier;
    }

    public bool Equals(ShortcutKey other)
    {
        return Key == other.Key && Modifiers == other.Modifiers;
    }

    public override bool Equals(object? obj)
    {
        return obj is ShortcutKey other && Equals(other);
    }

    public override int GetHashCode()
    {
        return HashCode.Combine(Key, Modifiers);
    }

    public static bool operator ==(ShortcutKey left, ShortcutKey right)
    {
        return left.Equals(right);
    }

    public static bool operator !=(ShortcutKey left, ShortcutKey right)
    {
        return !left.Equals(right);
    }

    public override string ToString()
    {
        return ShortcutManager.GetDisplayText(this);
    }
}

/// <summary>
/// Key codes for keyboard shortcuts (platform-agnostic).
/// </summary>
public enum KeyCode
{
    None = 0,
    A = 1, B = 2, C = 3, D = 4, E = 5, F = 6, G = 7, H = 8, I = 9, J = 10,
    K = 11, L = 12, M = 13, N = 14, O = 15, P = 16, Q = 17, R = 18, S = 19, T = 20,
    U = 21, V = 22, W = 23, X = 24, Y = 25, Z = 26,
    D0 = 30, D1 = 31, D2 = 32, D3 = 33, D4 = 34, D5 = 35, D6 = 36, D7 = 37, D8 = 38, D9 = 39,
    F1 = 40, F2 = 41, F3 = 42, F4 = 43, F5 = 44, F6 = 45, F7 = 46, F8 = 47, F9 = 48, F10 = 49, F11 = 50, F12 = 51,
    Enter = 60, Escape = 61, Space = 62, Tab = 63, Backspace = 64, Delete = 65, Insert = 66,
    Home = 67, End = 68, PageUp = 69, PageDown = 70,
    Left = 71, Right = 72, Up = 73, Down = 74,
    Plus = 80, Minus = 81
}

/// <summary>
/// Modifier keys for keyboard shortcuts.
/// </summary>
[Flags]
public enum KeyModifiers
{
    None = 0,
    Control = 1,
    Shift = 2,
    Alt = 4,
    Meta = 8
}
