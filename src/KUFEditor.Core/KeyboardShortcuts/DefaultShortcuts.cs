namespace KUFEditor.Core.KeyboardShortcuts;

/// <summary>
/// Default keyboard shortcuts for the editor.
/// </summary>
public static class DefaultShortcuts
{
    // File operations.
    public static ShortcutKey Save => new(KeyCode.S, KeyModifiers.Control);
    public static ShortcutKey SaveAll => new(KeyCode.S, KeyModifiers.Control | KeyModifiers.Shift);
    public static ShortcutKey Open => new(KeyCode.O, KeyModifiers.Control);
    public static ShortcutKey OpenFolder => new(KeyCode.O, KeyModifiers.Control | KeyModifiers.Shift);
    public static ShortcutKey New => new(KeyCode.N, KeyModifiers.Control);
    public static ShortcutKey Close => new(KeyCode.W, KeyModifiers.Control);

    // Edit operations.
    public static ShortcutKey Undo => new(KeyCode.Z, KeyModifiers.Control);
    public static ShortcutKey Redo => new(KeyCode.Y, KeyModifiers.Control);
    public static ShortcutKey RedoAlt => new(KeyCode.Z, KeyModifiers.Control | KeyModifiers.Shift);
    public static ShortcutKey Cut => new(KeyCode.X, KeyModifiers.Control);
    public static ShortcutKey Copy => new(KeyCode.C, KeyModifiers.Control);
    public static ShortcutKey Paste => new(KeyCode.V, KeyModifiers.Control);
    public static ShortcutKey SelectAll => new(KeyCode.A, KeyModifiers.Control);

    // View operations.
    public static ShortcutKey Refresh => new(KeyCode.F5, KeyModifiers.None);
    public static ShortcutKey ToggleInfoPanel => new(KeyCode.I, KeyModifiers.Control | KeyModifiers.Shift);
    public static ShortcutKey ToggleNavigator => new(KeyCode.E, KeyModifiers.Control | KeyModifiers.Shift);

    // Tab navigation.
    public static ShortcutKey NextTab => new(KeyCode.Tab, KeyModifiers.Control);
    public static ShortcutKey PreviousTab => new(KeyCode.Tab, KeyModifiers.Control | KeyModifiers.Shift);
    public static ShortcutKey CloseTab => new(KeyCode.W, KeyModifiers.Control);

    // Shortcut names for registration.
    public const string NameSave = "File.Save";
    public const string NameSaveAll = "File.SaveAll";
    public const string NameOpen = "File.Open";
    public const string NameOpenFolder = "File.OpenFolder";
    public const string NameNew = "File.New";
    public const string NameClose = "File.Close";
    public const string NameUndo = "Edit.Undo";
    public const string NameRedo = "Edit.Redo";
    public const string NameRedoAlt = "Edit.RedoAlt";
    public const string NameCut = "Edit.Cut";
    public const string NameCopy = "Edit.Copy";
    public const string NamePaste = "Edit.Paste";
    public const string NameSelectAll = "Edit.SelectAll";
    public const string NameRefresh = "View.Refresh";
    public const string NameToggleInfoPanel = "View.ToggleInfoPanel";
    public const string NameToggleNavigator = "View.ToggleNavigator";
    public const string NameNextTab = "Tab.Next";
    public const string NamePreviousTab = "Tab.Previous";
    public const string NameCloseTab = "Tab.Close";

    /// <summary>
    /// Registers all default shortcuts with placeholder actions.
    /// </summary>
    public static void RegisterDefaults(ShortcutManager manager, Action<string>? handler = null)
    {
        var defaultHandler = handler ?? (_ => { });

        manager.Register(new ShortcutBinding(NameSave, Save, () => defaultHandler(NameSave), "File")
        {
            Description = "Save the current file"
        });

        manager.Register(new ShortcutBinding(NameSaveAll, SaveAll, () => defaultHandler(NameSaveAll), "File")
        {
            Description = "Save all open files"
        });

        manager.Register(new ShortcutBinding(NameOpen, Open, () => defaultHandler(NameOpen), "File")
        {
            Description = "Open a file"
        });

        manager.Register(new ShortcutBinding(NameOpenFolder, OpenFolder, () => defaultHandler(NameOpenFolder), "File")
        {
            Description = "Open a folder"
        });

        manager.Register(new ShortcutBinding(NameNew, New, () => defaultHandler(NameNew), "File")
        {
            Description = "Create a new file"
        });

        manager.Register(new ShortcutBinding(NameUndo, Undo, () => defaultHandler(NameUndo), "Edit")
        {
            Description = "Undo the last action"
        });

        manager.Register(new ShortcutBinding(NameRedo, Redo, () => defaultHandler(NameRedo), "Edit")
        {
            Description = "Redo the last undone action"
        });

        manager.Register(new ShortcutBinding(NameRedoAlt, RedoAlt, () => defaultHandler(NameRedoAlt), "Edit")
        {
            Description = "Redo (alternative binding)"
        });

        manager.Register(new ShortcutBinding(NameCut, Cut, () => defaultHandler(NameCut), "Edit")
        {
            Description = "Cut selection"
        });

        manager.Register(new ShortcutBinding(NameCopy, Copy, () => defaultHandler(NameCopy), "Edit")
        {
            Description = "Copy selection"
        });

        manager.Register(new ShortcutBinding(NamePaste, Paste, () => defaultHandler(NamePaste), "Edit")
        {
            Description = "Paste from clipboard"
        });

        manager.Register(new ShortcutBinding(NameSelectAll, SelectAll, () => defaultHandler(NameSelectAll), "Edit")
        {
            Description = "Select all content"
        });

        manager.Register(new ShortcutBinding(NameRefresh, Refresh, () => defaultHandler(NameRefresh), "View")
        {
            Description = "Refresh the current view"
        });

        manager.Register(new ShortcutBinding(NameToggleInfoPanel, ToggleInfoPanel, () => defaultHandler(NameToggleInfoPanel), "View")
        {
            Description = "Toggle the info panel"
        });

        manager.Register(new ShortcutBinding(NameToggleNavigator, ToggleNavigator, () => defaultHandler(NameToggleNavigator), "View")
        {
            Description = "Toggle the workspace navigator"
        });

        manager.Register(new ShortcutBinding(NameNextTab, NextTab, () => defaultHandler(NameNextTab), "Tab")
        {
            Description = "Switch to next tab"
        });

        manager.Register(new ShortcutBinding(NamePreviousTab, PreviousTab, () => defaultHandler(NamePreviousTab), "Tab")
        {
            Description = "Switch to previous tab"
        });

        manager.Register(new ShortcutBinding(NameCloseTab, CloseTab, () => defaultHandler(NameCloseTab), "Tab")
        {
            Description = "Close current tab"
        });
    }
}
