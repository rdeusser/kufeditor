using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using KUFEditor.Assets.Patching;
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
    private readonly ModManager _modManager = null!;
    private readonly BackupManager _backupManager = null!;
    private readonly Settings _settings = null!;
    private readonly ObservableCollection<ModViewModel> _mods = new();
    private List<ModConflict> _lastConflicts = new();

    public ModManagerWindow()
    {
        InitializeComponent();
    }

    public ModManagerWindow(ModManager modManager, BackupManager backupManager, Settings settings, string initialGame = "Crusaders")
        : this()
    {
        _modManager = modManager;
        _backupManager = backupManager;
        _settings = settings;

        ModList.ItemsSource = _mods;
        ModList.SelectionChanged += OnModSelected;

        // Set initial game selection.
        for (int i = 0; i < GameSelector.Items.Count; i++)
        {
            if (GameSelector.Items[i] is ComboBoxItem item && item.Content?.ToString() == initialGame)
            {
                GameSelector.SelectedIndex = i;
                break;
            }
        }

        GameSelector.SelectionChanged += (s, e) => RefreshModList();
        ImportButton.Click += OnImport;
        CreateButton.Click += OnCreate;
        ApplyButton.Click += OnApply;
        DeleteButton.Click += OnDelete;
        MoveUpButton.Click += OnMoveUp;
        MoveDownButton.Click += OnMoveDown;
        ViewConflictsButton.Click += OnViewConflicts;

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
            .ThenBy(m => {
                var idx = enabled.IndexOf(m.Mod.Id);
                return idx >= 0 ? idx : int.MaxValue;
            })
            .ThenBy(m => m.Name)
            .ToList();

        _mods.Clear();
        foreach (var m in sorted) _mods.Add(m);

        UpdateConflictBar();
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

    private async void OnCreate(object? sender, RoutedEventArgs e)
    {
        var dialog = new ExportModDialog(SelectedGame, _backupManager, _settings);
        var result = await dialog.ShowDialog<bool>(this);

        if (result && dialog.ExportedMod != null)
        {
            // Import the newly created mod.
            _modManager.ImportMod(dialog.ExportedMod.SourcePath);
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
            ShowError("Game directory not configured. Please set it in Settings.");
            return;
        }

        var patcher = new SoxPatcherRegistry();
        var applier = new ModApplier(gameDir, _backupManager, patcher);
        applier.Apply(enabledMods, SelectedGame);

        _lastConflicts = applier.Conflicts;
        UpdateConflictBar();
    }

    private void UpdateConflictBar()
    {
        if (_lastConflicts.Count > 0)
        {
            ConflictBar.IsVisible = true;
            ConflictText.Text = $"{_lastConflicts.Count} conflict(s) detected";
        }
        else
        {
            ConflictBar.IsVisible = false;
        }
    }

    private async void OnViewConflicts(object? sender, RoutedEventArgs e)
    {
        if (_lastConflicts.Count == 0) return;

        var message = string.Join("\n", _lastConflicts.Select(c => c.ToString()));

        var dialog = new Window
        {
            Title = "Mod Conflicts",
            Width = 500,
            Height = 300,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = new ScrollViewer
            {
                Content = new TextBlock
                {
                    Text = message,
                    TextWrapping = Avalonia.Media.TextWrapping.Wrap,
                    Margin = new Avalonia.Thickness(16)
                }
            }
        };

        await dialog.ShowDialog(this);
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
                SelectMod(vm.Mod.Id);
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
                SelectMod(vm.Mod.Id);
            }
        }
    }

    private void SelectMod(string modId)
    {
        var vm = _mods.FirstOrDefault(m => m.Mod.Id == modId);
        if (vm != null)
        {
            ModList.SelectedItem = vm;
        }
    }

    private async void ShowError(string message)
    {
        var dialog = new Window
        {
            Title = "Error",
            Width = 400,
            Height = 150,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = new StackPanel
            {
                Margin = new Avalonia.Thickness(20),
                Children =
                {
                    new TextBlock
                    {
                        Text = message,
                        TextWrapping = Avalonia.Media.TextWrapping.Wrap
                    }
                }
            }
        };

        await dialog.ShowDialog(this);
    }
}
