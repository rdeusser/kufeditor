# Implementation Plan: Mod Manager

## Overview

Build a mod management system that allows users to create, install, order, and apply declarative JSON patches to game files.

## Prerequisites

- [x] Design document approved
- [ ] Branch created (optional - can work on main)

## Tasks

### Task 1: Create Mod Data Models

**Files:**
- Create: `src/KUFEditor.Core/Mods/Mod.cs`
- Create: `tests/KUFEditor.Core.Tests/ModTests.cs`

**Steps:**
1. Write test for Mod deserialization from JSON
2. Run test, verify it fails
3. Implement Mod, ModPatch, and PatchAction classes
4. Run test, verify it passes
5. Commit: "Add Mod data models"

**Code:**

```csharp
// src/KUFEditor.Core/Mods/Mod.cs
using System.Text.Json.Serialization;

namespace KUFEditor.Core.Mods;

public enum PatchAction
{
    Modify,
    Add,
    Delete
}

public class ModPatch
{
    public string File { get; set; } = string.Empty;
    public PatchAction Action { get; set; }
    public string? Record { get; set; }
    public Dictionary<string, object>? Fields { get; set; }
    public Dictionary<string, object>? Data { get; set; }
}

public class Mod
{
    public string Id { get; set; } = string.Empty;
    public string Name { get; set; } = string.Empty;
    public string Version { get; set; } = "1.0.0";
    public string Author { get; set; } = string.Empty;
    public string Description { get; set; } = string.Empty;
    public string Game { get; set; } = string.Empty;
    public List<ModPatch> Patches { get; set; } = new();

    public string SourcePath { get; set; } = string.Empty;
}
```

```csharp
// tests/KUFEditor.Core.Tests/ModTests.cs
using System.Text.Json;
using Xunit;
using KUFEditor.Core.Mods;

namespace KUFEditor.Core.Tests;

public class ModTests
{
    [Fact]
    public void Deserialize_ParsesModJson()
    {
        var json = """
        {
            "id": "test-mod",
            "name": "Test Mod",
            "version": "1.0.0",
            "author": "Tester",
            "description": "A test mod",
            "game": "Crusaders",
            "patches": [
                {
                    "file": "TroopInfo.sox",
                    "action": "Modify",
                    "record": "Gerald",
                    "fields": { "HP": 500 }
                }
            ]
        }
        """;

        var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
        var mod = JsonSerializer.Deserialize<Mod>(json, options);

        Assert.NotNull(mod);
        Assert.Equal("test-mod", mod.Id);
        Assert.Equal("Test Mod", mod.Name);
        Assert.Single(mod.Patches);
        Assert.Equal("TroopInfo.sox", mod.Patches[0].File);
        Assert.Equal(PatchAction.Modify, mod.Patches[0].Action);
    }
}
```

**Verify:**
```bash
dotnet test --filter "ModTests"
# Expected: 1 passing
```

---

### Task 2: Create ModManager for State

**Files:**
- Create: `src/KUFEditor.Core/Mods/ModManager.cs`
- Modify: `tests/KUFEditor.Core.Tests/ModTests.cs`

**Steps:**
1. Write tests for ModManager load/save state and mod enumeration
2. Run tests, verify they fail
3. Implement ModManager
4. Run tests, verify they pass
5. Commit: "Add ModManager for state management"

**Code:**

```csharp
// src/KUFEditor.Core/Mods/ModManager.cs
using System.IO.Compression;
using System.Text.Json;

namespace KUFEditor.Core.Mods;

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
        using var zip = ZipFile.OpenRead(path);
        var entry = zip.GetEntry("mod.json");
        if (entry == null) return null;

        using var stream = entry.Open();
        using var reader = new StreamReader(stream);
        var json = reader.ReadToEnd();

        var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
        return JsonSerializer.Deserialize<Mod>(json, options);
    }

    public List<string> GetEnabledMods(string game)
    {
        return _state.TryGetValue(game, out var list) ? list : new List<string>();
    }

    public void SetEnabledMods(string game, List<string> modIds)
    {
        _state[game] = modIds;
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
}
```

```csharp
// Add to tests/KUFEditor.Core.Tests/ModTests.cs

public class ModManagerTests
{
    private string GetTempDir() => Path.Combine(Path.GetTempPath(), $"modtest_{Guid.NewGuid()}");

    [Fact]
    public void GetEnabledMods_ReturnsEmptyForNewGame()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            var enabled = manager.GetEnabledMods("Crusaders");
            Assert.Empty(enabled);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void EnableMod_AddToList()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            manager.EnableMod("Crusaders", "test-mod");

            Assert.True(manager.IsEnabled("Crusaders", "test-mod"));
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SaveAndLoad_PersistsState()
    {
        var dir = GetTempDir();
        try
        {
            var manager1 = new ModManager(dir);
            manager1.EnableMod("Crusaders", "mod-a");
            manager1.EnableMod("Crusaders", "mod-b");

            var manager2 = new ModManager(dir);
            var enabled = manager2.GetEnabledMods("Crusaders");

            Assert.Equal(2, enabled.Count);
            Assert.Equal("mod-a", enabled[0]);
            Assert.Equal("mod-b", enabled[1]);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void ReorderMod_ChangesPosition()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            manager.EnableMod("Crusaders", "mod-a");
            manager.EnableMod("Crusaders", "mod-b");
            manager.EnableMod("Crusaders", "mod-c");

            manager.ReorderMod("Crusaders", "mod-c", 0);

            var enabled = manager.GetEnabledMods("Crusaders");
            Assert.Equal("mod-c", enabled[0]);
            Assert.Equal("mod-a", enabled[1]);
            Assert.Equal("mod-b", enabled[2]);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }
}
```

**Verify:**
```bash
dotnet test --filter "ModManagerTests"
# Expected: 4 passing
```

---

### Task 3: Create ModApplier with Conflict Detection

**Files:**
- Create: `src/KUFEditor.Core/Mods/ModApplier.cs`
- Create: `src/KUFEditor.Core/Mods/ModConflict.cs`
- Modify: `tests/KUFEditor.Core.Tests/ModTests.cs`

**Steps:**
1. Write tests for conflict detection
2. Run tests, verify they fail
3. Implement ModApplier and ModConflict
4. Run tests, verify they pass
5. Commit: "Add ModApplier with conflict detection"

**Code:**

```csharp
// src/KUFEditor.Core/Mods/ModConflict.cs
namespace KUFEditor.Core.Mods;

public class ModConflict
{
    public string File { get; set; } = string.Empty;
    public string Record { get; set; } = string.Empty;
    public string Field { get; set; } = string.Empty;
    public string FirstModId { get; set; } = string.Empty;
    public object? FirstValue { get; set; }
    public string SecondModId { get; set; } = string.Empty;
    public object? SecondValue { get; set; }
}
```

```csharp
// src/KUFEditor.Core/Mods/ModApplier.cs
namespace KUFEditor.Core.Mods;

public class ModApplier
{
    private readonly BackupManager _backupManager;
    private readonly string _gameDirectory;
    private readonly Dictionary<string, (string modId, object? value)> _touchedFields = new();

    public List<ModConflict> Conflicts { get; } = new();

    public ModApplier(BackupManager backupManager, string gameDirectory)
    {
        _backupManager = backupManager;
        _gameDirectory = gameDirectory;
    }

    public void Apply(List<Mod> mods)
    {
        _touchedFields.Clear();
        Conflicts.Clear();

        // Restore from pristine first.
        RestoreFromPristine(mods);

        // Apply each mod in order.
        foreach (var mod in mods)
        {
            ApplyMod(mod);
        }
    }

    private void RestoreFromPristine(List<Mod> mods)
    {
        var filesToRestore = mods
            .SelectMany(m => m.Patches)
            .Select(p => p.File)
            .Distinct();

        foreach (var file in filesToRestore)
        {
            var gamePath = Path.Combine(_gameDirectory, file);
            _backupManager.RestorePristine(gamePath);
        }
    }

    private void ApplyMod(Mod mod)
    {
        foreach (var patch in mod.Patches)
        {
            ApplyPatch(mod.Id, patch);
        }
    }

    private void ApplyPatch(string modId, ModPatch patch)
    {
        // Track field touches for conflict detection.
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

        // Actual file patching will be implemented when we integrate with SOX parsers.
        // For now, just track conflicts.
    }
}
```

```csharp
// Add to tests/KUFEditor.Core.Tests/ModTests.cs

public class ModApplierTests
{
    [Fact]
    public void Apply_DetectsFieldConflicts()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"applier_{Guid.NewGuid()}");
        Directory.CreateDirectory(tempDir);

        try
        {
            var backupManager = new BackupManager(tempDir);
            var applier = new ModApplier(backupManager, tempDir);

            var mod1 = new Mod
            {
                Id = "mod-a",
                Patches = new List<ModPatch>
                {
                    new() { File = "TroopInfo.sox", Action = PatchAction.Modify, Record = "Gerald",
                            Fields = new() { { "HP", 500 } } }
                }
            };

            var mod2 = new Mod
            {
                Id = "mod-b",
                Patches = new List<ModPatch>
                {
                    new() { File = "TroopInfo.sox", Action = PatchAction.Modify, Record = "Gerald",
                            Fields = new() { { "HP", 600 } } }
                }
            };

            applier.Apply(new List<Mod> { mod1, mod2 });

            Assert.Single(applier.Conflicts);
            Assert.Equal("TroopInfo.sox", applier.Conflicts[0].File);
            Assert.Equal("Gerald", applier.Conflicts[0].Record);
            Assert.Equal("HP", applier.Conflicts[0].Field);
            Assert.Equal("mod-a", applier.Conflicts[0].FirstModId);
            Assert.Equal("mod-b", applier.Conflicts[0].SecondModId);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Apply_NoConflictForDifferentFields()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"applier_{Guid.NewGuid()}");
        Directory.CreateDirectory(tempDir);

        try
        {
            var backupManager = new BackupManager(tempDir);
            var applier = new ModApplier(backupManager, tempDir);

            var mod1 = new Mod
            {
                Id = "mod-a",
                Patches = new List<ModPatch>
                {
                    new() { File = "TroopInfo.sox", Action = PatchAction.Modify, Record = "Gerald",
                            Fields = new() { { "HP", 500 } } }
                }
            };

            var mod2 = new Mod
            {
                Id = "mod-b",
                Patches = new List<ModPatch>
                {
                    new() { File = "TroopInfo.sox", Action = PatchAction.Modify, Record = "Gerald",
                            Fields = new() { { "Attack", 75 } } }
                }
            };

            applier.Apply(new List<Mod> { mod1, mod2 });

            Assert.Empty(applier.Conflicts);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
        }
    }
}
```

**Verify:**
```bash
dotnet test --filter "ModApplierTests"
# Expected: 2 passing
```

---

### Task 4: Create ModManagerWindow UI

**Files:**
- Create: `src/KUFEditor/UI/Dialogs/ModManagerWindow.axaml`
- Create: `src/KUFEditor/UI/Dialogs/ModManagerWindow.axaml.cs`

**Steps:**
1. Create AXAML layout matching design mockup
2. Implement code-behind with mod list binding
3. Wire up Import, Apply, Delete buttons
4. Verify window opens and displays correctly
5. Commit: "Add ModManagerWindow UI"

**Code:**

```xml
<!-- src/KUFEditor/UI/Dialogs/ModManagerWindow.axaml -->
<Window xmlns="https://github.com/avaloniaui"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        x:Class="KUFEditor.UI.Dialogs.ModManagerWindow"
        Title="Mod Manager"
        Width="700" Height="500"
        WindowStartupLocation="CenterOwner">

    <Grid RowDefinitions="Auto,*,Auto">
        <!-- Header -->
        <Grid Grid.Row="0" ColumnDefinitions="*,Auto" Margin="12">
            <ComboBox x:Name="GameSelector" Width="150">
                <ComboBoxItem Content="Crusaders" IsSelected="True"/>
                <ComboBoxItem Content="Heroes"/>
            </ComboBox>
            <StackPanel Grid.Column="1" Orientation="Horizontal" Spacing="8">
                <Button x:Name="ImportButton" Content="Import"/>
                <Button x:Name="CreateButton" Content="Create"/>
                <Button x:Name="ApplyButton" Content="Apply" Classes="accent"/>
            </StackPanel>
        </Grid>

        <!-- Content -->
        <Grid Grid.Row="1" ColumnDefinitions="250,*" Margin="12,0">
            <!-- Mod List -->
            <Border Background="{DynamicResource CardBackgroundFillColorDefaultBrush}"
                    CornerRadius="8" Padding="8">
                <Grid RowDefinitions="*,Auto">
                    <ListBox x:Name="ModList" SelectionMode="Single">
                        <ListBox.ItemTemplate>
                            <DataTemplate>
                                <StackPanel Orientation="Horizontal" Spacing="8">
                                    <CheckBox IsChecked="{Binding IsEnabled}"/>
                                    <TextBlock Text="{Binding Name}" VerticalAlignment="Center"/>
                                </StackPanel>
                            </DataTemplate>
                        </ListBox.ItemTemplate>
                    </ListBox>
                    <StackPanel Grid.Row="1" Orientation="Horizontal" Spacing="4" Margin="0,8,0,0">
                        <Button x:Name="MoveUpButton" Content="Up" Padding="12,4"/>
                        <Button x:Name="MoveDownButton" Content="Down" Padding="12,4"/>
                    </StackPanel>
                </Grid>
            </Border>

            <!-- Details Panel -->
            <Border Grid.Column="1" Background="{DynamicResource CardBackgroundFillColorDefaultBrush}"
                    CornerRadius="8" Padding="16" Margin="12,0,0,0">
                <Grid RowDefinitions="Auto,Auto,*,Auto" x:Name="DetailsPanel">
                    <StackPanel>
                        <TextBlock x:Name="ModName" FontSize="18" FontWeight="Bold"/>
                        <TextBlock x:Name="ModVersion" Foreground="{DynamicResource TextFillColorSecondaryBrush}"/>
                        <TextBlock x:Name="ModAuthor" Foreground="{DynamicResource TextFillColorSecondaryBrush}" Margin="0,0,0,12"/>
                    </StackPanel>
                    <TextBlock Grid.Row="1" x:Name="ModDescription" TextWrapping="Wrap" Margin="0,0,0,12"/>
                    <StackPanel Grid.Row="2" x:Name="FilesModified">
                        <TextBlock Text="Files modified:" FontWeight="SemiBold" Margin="0,0,0,4"/>
                        <ItemsControl x:Name="FilesList"/>
                    </StackPanel>
                    <Button Grid.Row="3" x:Name="DeleteButton" Content="Delete Mod"
                            HorizontalAlignment="Left" Margin="0,12,0,0"/>
                </Grid>
            </Border>
        </Grid>

        <!-- Conflict Bar -->
        <Border Grid.Row="2" x:Name="ConflictBar" Background="#FFF3CD"
                IsVisible="False" Padding="12" Margin="12">
            <Grid ColumnDefinitions="*,Auto">
                <TextBlock x:Name="ConflictText" VerticalAlignment="Center"/>
                <Button Grid.Column="1" x:Name="ViewConflictsButton" Content="View"/>
            </Grid>
        </Border>
    </Grid>
</Window>
```

```csharp
// src/KUFEditor/UI/Dialogs/ModManagerWindow.axaml.cs
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using KUFEditor.Core;
using KUFEditor.Core.Mods;

namespace KUFEditor.UI.Dialogs;

public class ModViewModel
{
    public Mod Mod { get; set; } = null!;
    public string Name => Mod.Name;
    public bool IsEnabled { get; set; }
}

public partial class ModManagerWindow : Window
{
    private readonly ModManager _modManager;
    private readonly BackupManager _backupManager;
    private readonly Settings _settings;
    private readonly ObservableCollection<ModViewModel> _mods = new();

    public ModManagerWindow(ModManager modManager, BackupManager backupManager, Settings settings)
    {
        InitializeComponent();

        _modManager = modManager;
        _backupManager = backupManager;
        _settings = settings;

        ModList.ItemsSource = _mods;
        ModList.SelectionChanged += OnModSelected;

        GameSelector.SelectionChanged += (s, e) => RefreshModList();
        ImportButton.Click += OnImport;
        ApplyButton.Click += OnApply;
        DeleteButton.Click += OnDelete;
        MoveUpButton.Click += OnMoveUp;
        MoveDownButton.Click += OnMoveDown;

        RefreshModList();
    }

    private string SelectedGame => (GameSelector.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "Crusaders";

    private void RefreshModList()
    {
        _mods.Clear();
        var enabled = _modManager.GetEnabledMods(SelectedGame);

        foreach (var mod in _modManager.InstalledMods.Where(m => m.Game == SelectedGame))
        {
            _mods.Add(new ModViewModel
            {
                Mod = mod,
                IsEnabled = enabled.Contains(mod.Id)
            });
        }

        // Sort by enabled order, then alphabetically.
        var sorted = _mods
            .OrderByDescending(m => m.IsEnabled)
            .ThenBy(m => enabled.IndexOf(m.Mod.Id))
            .ThenBy(m => m.Name)
            .ToList();

        _mods.Clear();
        foreach (var m in sorted) _mods.Add(m);
    }

    private void OnModSelected(object? sender, SelectionChangedEventArgs e)
    {
        if (ModList.SelectedItem is ModViewModel vm)
        {
            DetailsPanel.IsVisible = true;
            ModName.Text = vm.Mod.Name;
            ModVersion.Text = $"v{vm.Mod.Version}";
            ModAuthor.Text = $"by {vm.Mod.Author}";
            ModDescription.Text = vm.Mod.Description;

            var files = vm.Mod.Patches
                .GroupBy(p => p.File)
                .Select(g => $"- {g.Key} ({g.Count()} patches)");
            FilesList.ItemsSource = files;
        }
        else
        {
            DetailsPanel.IsVisible = false;
        }
    }

    private async void OnImport(object? sender, RoutedEventArgs e)
    {
        var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Import Mod",
            AllowMultiple = false,
            FileTypeFilter = new[] { new FilePickerFileType("KUF Mod") { Patterns = new[] { "*.kufmod" } } }
        });

        if (files.Count > 0)
        {
            _modManager.ImportMod(files[0].Path.LocalPath);
            RefreshModList();
        }
    }

    private void OnApply(object? sender, RoutedEventArgs e)
    {
        var enabledIds = _mods.Where(m => m.IsEnabled).Select(m => m.Mod.Id).ToList();
        _modManager.SetEnabledMods(SelectedGame, enabledIds);

        var enabledMods = _mods.Where(m => m.IsEnabled).Select(m => m.Mod).ToList();
        var gameDir = SelectedGame == "Crusaders" ? _settings.CrusadersPath : _settings.HeroesPath;

        if (string.IsNullOrEmpty(gameDir))
        {
            // Show error.
            return;
        }

        var applier = new ModApplier(_backupManager, gameDir);
        applier.Apply(enabledMods);

        if (applier.Conflicts.Any())
        {
            ConflictBar.IsVisible = true;
            ConflictText.Text = $"{applier.Conflicts.Count} conflict(s) detected";
        }
        else
        {
            ConflictBar.IsVisible = false;
        }
    }

    private void OnDelete(object? sender, RoutedEventArgs e)
    {
        if (ModList.SelectedItem is ModViewModel vm)
        {
            _modManager.DeleteMod(vm.Mod);
            RefreshModList();
        }
    }

    private void OnMoveUp(object? sender, RoutedEventArgs e)
    {
        if (ModList.SelectedItem is ModViewModel vm && vm.IsEnabled)
        {
            var enabled = _modManager.GetEnabledMods(SelectedGame);
            var idx = enabled.IndexOf(vm.Mod.Id);
            if (idx > 0)
            {
                _modManager.ReorderMod(SelectedGame, vm.Mod.Id, idx - 1);
                RefreshModList();
            }
        }
    }

    private void OnMoveDown(object? sender, RoutedEventArgs e)
    {
        if (ModList.SelectedItem is ModViewModel vm && vm.IsEnabled)
        {
            var enabled = _modManager.GetEnabledMods(SelectedGame);
            var idx = enabled.IndexOf(vm.Mod.Id);
            if (idx < enabled.Count - 1)
            {
                _modManager.ReorderMod(SelectedGame, vm.Mod.Id, idx + 1);
                RefreshModList();
            }
        }
    }
}
```

**Verify:**
```bash
dotnet build
dotnet run --project src/KUFEditor
# Open Tools > Mod Manager, verify window appears
```

---

### Task 5: Wire Up ModManagerWindow to Main Menu

**Files:**
- Modify: `src/KUFEditor/UI/KUFEditor.axaml`
- Modify: `src/KUFEditor/UI/KUFEditor.axaml.cs`

**Steps:**
1. Add "Mod Manager..." menu item to Tools menu
2. Add click handler that opens ModManagerWindow
3. Verify menu opens the window
4. Commit: "Wire up Mod Manager to Tools menu"

**Code:**

```xml
<!-- Add to Tools menu in src/KUFEditor/UI/KUFEditor.axaml -->
<MenuItem Header="Tools">
    <MenuItem x:Name="ModManagerMenuItem" Header="Mod Manager..."/>
    <Separator/>
    <MenuItem x:Name="BackupAllMenuItem" Header="Backup All"/>
    <!-- ... existing items ... -->
</MenuItem>
```

```csharp
// Add to SetupMenuItems() in src/KUFEditor/UI/KUFEditor.axaml.cs
var modManagerMenuItem = this.FindControl<MenuItem>("ModManagerMenuItem");
if (modManagerMenuItem != null) modManagerMenuItem.Click += OnOpenModManager;

// Add method:
private async void OnOpenModManager(object? sender, RoutedEventArgs e)
{
    var modsDir = Path.Combine(Settings.GetDefaultSettingsPath(), "..", "mods");
    var modManager = new ModManager(modsDir);
    var window = new ModManagerWindow(modManager, backupManager!, settings);
    await window.ShowDialog(this);
}
```

**Verify:**
```bash
dotnet run --project src/KUFEditor
# Click Tools > Mod Manager..., verify window opens
```

---

### Task 6: Add Export Mod from Editor

**Files:**
- Modify: `src/KUFEditor/UI/KUFEditor.axaml`
- Modify: `src/KUFEditor/UI/KUFEditor.axaml.cs`
- Create: `src/KUFEditor/UI/Dialogs/ExportModDialog.axaml`
- Create: `src/KUFEditor/UI/Dialogs/ExportModDialog.axaml.cs`

**Steps:**
1. Add "Export as Mod..." to File menu
2. Create dialog for mod metadata (name, author, description)
3. Implement diff generation from pristine to current
4. Package as .kufmod ZIP
5. Commit: "Add Export as Mod feature"

This task is more complex and should be broken into subtasks during implementation.

---

## Execution

Ready to execute with subagent-driven development or save for a separate session.
