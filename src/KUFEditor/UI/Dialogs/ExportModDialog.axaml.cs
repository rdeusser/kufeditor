using System;
using System.IO;
using System.IO.Compression;
using System.Text.Json;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using KUFEditor.Core.Mods;

namespace KUFEditor.UI.Dialogs;

public partial class ExportModDialog : Window
{
    private readonly string _game;

    public Mod? ExportedMod { get; private set; }

    public ExportModDialog() : this("Crusaders") { }

    public ExportModDialog(string game)
    {
        InitializeComponent();
        _game = game;

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

        // Create the mod object.
        ExportedMod = new Mod
        {
            Id = id,
            Name = name,
            Version = version,
            Author = author,
            Description = description,
            Game = _game,
            Patches = new() // TODO: Generate patches from editor changes.
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
