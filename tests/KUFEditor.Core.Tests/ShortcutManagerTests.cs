using Xunit;
using KUFEditor.Core.KeyboardShortcuts;

namespace KUFEditor.Core.Tests;

public class ShortcutManagerTests
{
    [Fact]
    public void Register_AddsBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { });

        manager.Register(binding);

        Assert.Single(manager.Bindings);
    }

    [Fact]
    public void GetBinding_ReturnsRegisteredBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { });
        manager.Register(binding);

        var result = manager.GetBinding("Save");

        Assert.Equal(binding, result);
    }

    [Fact]
    public void GetBinding_ReturnsNullForUnknown()
    {
        var manager = new ShortcutManager();

        var result = manager.GetBinding("Unknown");

        Assert.Null(result);
    }

    [Fact]
    public void TryHandle_ExecutesMatchingBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var executed = false;
        var binding = new ShortcutBinding("Save", key, () => executed = true);
        manager.Register(binding);

        var result = manager.TryHandle(key);

        Assert.True(result);
        Assert.True(executed);
    }

    [Fact]
    public void TryHandle_ReturnsFalseForUnknownKey()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);

        var result = manager.TryHandle(key);

        Assert.False(result);
    }

    [Fact]
    public void TryHandle_DoesNotExecuteDisabledBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var executed = false;
        var binding = new ShortcutBinding("Save", key, () => executed = true)
        {
            IsEnabled = false
        };
        manager.Register(binding);

        var result = manager.TryHandle(key);

        Assert.False(result);
        Assert.False(executed);
    }

    [Fact]
    public void TryHandle_RaisesShortcutExecutedEvent()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { });
        manager.Register(binding);

        ShortcutBinding? executedBinding = null;
        manager.ShortcutExecuted += (s, e) => executedBinding = e.Binding;

        manager.TryHandle(key);

        Assert.Equal(binding, executedBinding);
    }

    [Fact]
    public void Unregister_ByKey_RemovesBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { });
        manager.Register(binding);

        var result = manager.Unregister(key);

        Assert.True(result);
        Assert.Empty(manager.Bindings);
    }

    [Fact]
    public void Unregister_ByName_RemovesBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { });
        manager.Register(binding);

        var result = manager.Unregister("Save");

        Assert.True(result);
        Assert.Empty(manager.Bindings);
    }

    [Fact]
    public void Unregister_ReturnsFalseForUnknown()
    {
        var manager = new ShortcutManager();

        Assert.False(manager.Unregister(new ShortcutKey(KeyCode.S, KeyModifiers.Control)));
        Assert.False(manager.Unregister("Unknown"));
    }

    [Fact]
    public void Clear_RemovesAllBindings()
    {
        var manager = new ShortcutManager();
        manager.Register(new ShortcutBinding("A", new ShortcutKey(KeyCode.A, KeyModifiers.Control), () => { }));
        manager.Register(new ShortcutBinding("B", new ShortcutKey(KeyCode.B, KeyModifiers.Control), () => { }));

        manager.Clear();

        Assert.Empty(manager.Bindings);
    }

    [Fact]
    public void Register_OverwritesExistingKeyBinding()
    {
        var manager = new ShortcutManager();
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        manager.Register(new ShortcutBinding("OldSave", key, () => { }));

        var newBinding = new ShortcutBinding("NewSave", key, () => { });
        manager.Register(newBinding);

        Assert.Single(manager.Bindings);
        Assert.Equal("NewSave", manager.Bindings.First().Name);
    }

    [Fact]
    public void GetDisplayText_FormatsCorrectly()
    {
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var text = ShortcutManager.GetDisplayText(key);

        Assert.Contains("S", text);
        Assert.True(text.Contains("Ctrl") || text.Contains("⌘"));
    }

    [Fact]
    public void GetDisplayText_HandlesMultipleModifiers()
    {
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control | KeyModifiers.Shift);
        var text = ShortcutManager.GetDisplayText(key);

        Assert.Contains("S", text);
        Assert.True(text.Contains("Shift") || text.Contains("⇧"));
    }
}

public class ShortcutKeyTests
{
    [Fact]
    public void Equals_SameKey_ReturnsTrue()
    {
        var key1 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var key2 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);

        Assert.Equal(key1, key2);
        Assert.True(key1 == key2);
    }

    [Fact]
    public void Equals_DifferentKey_ReturnsFalse()
    {
        var key1 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var key2 = new ShortcutKey(KeyCode.A, KeyModifiers.Control);

        Assert.NotEqual(key1, key2);
        Assert.True(key1 != key2);
    }

    [Fact]
    public void Equals_DifferentModifiers_ReturnsFalse()
    {
        var key1 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var key2 = new ShortcutKey(KeyCode.S, KeyModifiers.Shift);

        Assert.NotEqual(key1, key2);
    }

    [Fact]
    public void HasModifier_ReturnsCorrectly()
    {
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control | KeyModifiers.Shift);

        Assert.True(key.HasModifier(KeyModifiers.Control));
        Assert.True(key.HasModifier(KeyModifiers.Shift));
        Assert.False(key.HasModifier(KeyModifiers.Alt));
    }

    [Fact]
    public void GetHashCode_SameForEqualKeys()
    {
        var key1 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var key2 = new ShortcutKey(KeyCode.S, KeyModifiers.Control);

        Assert.Equal(key1.GetHashCode(), key2.GetHashCode());
    }

    [Fact]
    public void ToString_ReturnsDisplayText()
    {
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var text = key.ToString();

        Assert.Contains("S", text);
    }
}

public class ShortcutBindingTests
{
    [Fact]
    public void Constructor_SetsProperties()
    {
        var key = new ShortcutKey(KeyCode.S, KeyModifiers.Control);
        var binding = new ShortcutBinding("Save", key, () => { }, "File");

        Assert.Equal("Save", binding.Name);
        Assert.Equal(key, binding.Key);
        Assert.Equal("File", binding.Category);
        Assert.True(binding.IsEnabled);
    }

    [Fact]
    public void Constructor_ThrowsOnNullName()
    {
        Assert.Throws<ArgumentNullException>(() =>
            new ShortcutBinding(null!, new ShortcutKey(KeyCode.S, KeyModifiers.Control), () => { }));
    }

    [Fact]
    public void Constructor_ThrowsOnNullAction()
    {
        Assert.Throws<ArgumentNullException>(() =>
            new ShortcutBinding("Save", new ShortcutKey(KeyCode.S, KeyModifiers.Control), null!));
    }

    [Fact]
    public void Execute_CallsAction()
    {
        var executed = false;
        var binding = new ShortcutBinding("Test", new ShortcutKey(KeyCode.T, KeyModifiers.None), () => executed = true);

        binding.Execute();

        Assert.True(executed);
    }

    [Fact]
    public void Execute_DoesNotCallWhenDisabled()
    {
        var executed = false;
        var binding = new ShortcutBinding("Test", new ShortcutKey(KeyCode.T, KeyModifiers.None), () => executed = true)
        {
            IsEnabled = false
        };

        binding.Execute();

        Assert.False(executed);
    }

    [Fact]
    public void Description_CanBeSet()
    {
        var binding = new ShortcutBinding("Test", new ShortcutKey(KeyCode.T, KeyModifiers.None), () => { });

        binding.Description = "Test description";

        Assert.Equal("Test description", binding.Description);
    }
}

public class DefaultShortcutsTests
{
    [Fact]
    public void DefaultKeys_AreUnique()
    {
        var keys = new List<ShortcutKey>
        {
            DefaultShortcuts.Save,
            DefaultShortcuts.SaveAll,
            DefaultShortcuts.Open,
            DefaultShortcuts.OpenFolder,
            DefaultShortcuts.New,
            DefaultShortcuts.Undo,
            DefaultShortcuts.Redo,
            DefaultShortcuts.Cut,
            DefaultShortcuts.Copy,
            DefaultShortcuts.Paste,
            DefaultShortcuts.SelectAll,
            DefaultShortcuts.Refresh,
            DefaultShortcuts.ToggleInfoPanel,
            DefaultShortcuts.ToggleNavigator,
            DefaultShortcuts.NextTab,
            DefaultShortcuts.PreviousTab
        };

        var unique = keys.Distinct().ToList();
        Assert.Equal(keys.Count, unique.Count);
    }

    [Fact]
    public void RegisterDefaults_RegistersAllShortcuts()
    {
        var manager = new ShortcutManager();

        DefaultShortcuts.RegisterDefaults(manager);

        Assert.True(manager.Bindings.Count >= 15);
    }

    [Fact]
    public void RegisterDefaults_CallsHandlerOnExecution()
    {
        var manager = new ShortcutManager();
        var calledWith = string.Empty;
        DefaultShortcuts.RegisterDefaults(manager, name => calledWith = name);

        manager.TryHandle(DefaultShortcuts.Save);

        Assert.Equal(DefaultShortcuts.NameSave, calledWith);
    }

    [Fact]
    public void Save_HasControlModifier()
    {
        Assert.True(DefaultShortcuts.Save.HasModifier(KeyModifiers.Control));
        Assert.Equal(KeyCode.S, DefaultShortcuts.Save.Key);
    }

    [Fact]
    public void SaveAll_HasControlShiftModifiers()
    {
        Assert.True(DefaultShortcuts.SaveAll.HasModifier(KeyModifiers.Control));
        Assert.True(DefaultShortcuts.SaveAll.HasModifier(KeyModifiers.Shift));
        Assert.Equal(KeyCode.S, DefaultShortcuts.SaveAll.Key);
    }

    [Fact]
    public void Undo_HasControlZ()
    {
        Assert.True(DefaultShortcuts.Undo.HasModifier(KeyModifiers.Control));
        Assert.Equal(KeyCode.Z, DefaultShortcuts.Undo.Key);
    }

    [Fact]
    public void Redo_HasControlY()
    {
        Assert.True(DefaultShortcuts.Redo.HasModifier(KeyModifiers.Control));
        Assert.Equal(KeyCode.Y, DefaultShortcuts.Redo.Key);
    }

    [Fact]
    public void RedoAlt_HasControlShiftZ()
    {
        Assert.True(DefaultShortcuts.RedoAlt.HasModifier(KeyModifiers.Control));
        Assert.True(DefaultShortcuts.RedoAlt.HasModifier(KeyModifiers.Shift));
        Assert.Equal(KeyCode.Z, DefaultShortcuts.RedoAlt.Key);
    }

    [Fact]
    public void Refresh_UsesF5()
    {
        Assert.Equal(KeyCode.F5, DefaultShortcuts.Refresh.Key);
        Assert.Equal(KeyModifiers.None, DefaultShortcuts.Refresh.Modifiers);
    }
}
