using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Text.Json;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using KUFEditor.Assets.Patching;
using KUFEditor.Core;
using KUFEditor.Core.Mods;

namespace KUFEditor.UI.Dialogs;

public partial class ExportModDialog : Window
{
    private readonly string _game;
    private readonly BackupManager? _backupManager;
    private readonly Settings? _settings;

    public Mod? ExportedMod { get; private set; }

    public ExportModDialog() : this("Crusaders") { }

    public ExportModDialog(string game) : this(game, null, null) { }

    public ExportModDialog(string game, BackupManager? backupManager, Settings? settings)
    {
        InitializeComponent();
        _game = game;
        _backupManager = backupManager;
        _settings = settings;

        CancelButton.Click += (s, e) => Close(false);
        ExportButton.Click += OnExport;
    }

    private async void OnExport(object? sender, RoutedEventArgs e)
    {
        var id = ModIdTextBox.Text?.Trim();
        var name = ModNameTextBox.Text?.Trim();
        var version = ModVersionTextBox.Text?.Trim() ?? "1.0.0";
        var author = ModAuthorTextBox.Text?.Trim() ?? "";
        var description = ModDescriptionTextBox.Text?.Trim() ?? "";

        if (string.IsNullOrEmpty(id))
        {
            await ShowError("Mod ID is required.");
            return;
        }

        if (string.IsNullOrEmpty(name))
        {
            await ShowError("Mod Name is required.");
            return;
        }

        // Generate patches by comparing pristine vs current files.
        var patches = GeneratePatches();

        if (patches.Count == 0)
        {
            await ShowError("No changes detected. Make some edits before exporting a mod.");
            return;
        }

        // Create the mod object.
        ExportedMod = new Mod
        {
            Id = id,
            Name = name,
            Version = version,
            Author = author,
            Description = description,
            Game = _game,
            Patches = patches
        };

        // Ask user where to save.
        var file = await StorageProvider.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Save Mod",
            DefaultExtension = "kufmod",
            SuggestedFileName = $"{id}.kufmod",
            FileTypeChoices = new[] { new FilePickerFileType("KUF Mod") { Patterns = new[] { "*.kufmod" } } }
        });

        if (file == null)
        {
            return;
        }

        try
        {
            var path = file.Path.LocalPath;

            // Create the .kufmod ZIP file.
            using var stream = new FileStream(path, FileMode.Create);
            using var zip = new ZipArchive(stream, ZipArchiveMode.Create);

            // Add mod.json.
            var entry = zip.CreateEntry("mod.json");
            using var entryStream = entry.Open();
            var options = new JsonSerializerOptions { WriteIndented = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
            await JsonSerializer.SerializeAsync(entryStream, ExportedMod, options);

            ExportedMod.SourcePath = path;
            Close(true);
        }
        catch (Exception ex)
        {
            await ShowError($"Failed to export mod: {ex.Message}");
        }
    }

    private List<ModPatch> GeneratePatches()
    {
        var patches = new List<ModPatch>();

        if (_backupManager == null || _settings == null)
            return patches;

        var gameDir = _game == "Crusaders" ? _settings.CrusadersPath : _settings.HeroesPath;
        if (string.IsNullOrEmpty(gameDir))
            return patches;

        var generator = new DiffGeneratorRegistry();

        // Compare SOX files.
        var soxDir = Path.Combine(gameDir, "Data", "SOX");
        if (!Directory.Exists(soxDir))
            soxDir = Path.Combine(gameDir, "Data", "Sox");

        if (Directory.Exists(soxDir))
        {
            foreach (var currentFile in Directory.GetFiles(soxDir, "*.sox"))
            {
                var fileName = Path.GetFileName(currentFile);

                if (!generator.CanHandle(fileName))
                    continue;

                var pristinePath = _backupManager.GetPristinePath(_game, fileName);
                if (pristinePath == null || !File.Exists(pristinePath))
                    continue;

                var relativePath = Path.Combine("Data", "SOX", fileName);
                var filePatches = generator.GenerateDiff(pristinePath, currentFile, relativePath);
                patches.AddRange(filePatches);
            }
        }

        return patches;
    }

    private async System.Threading.Tasks.Task ShowError(string message)
    {
        var dialog = new Window
        {
            Title = "Error",
            Width = 350,
            Height = 120,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = new StackPanel
            {
                Margin = new Avalonia.Thickness(16),
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
