using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using KUFEditor.Core;

namespace KUFEditor.UI.Views;

public partial class WorkspaceNavigator : UserControl
{
    private Settings? _settings;
    private string? _currentGame;

    public ObservableCollection<FileItem> SoxItems { get; } = new();
    public ObservableCollection<FileItem> MissionItems { get; } = new();
    public ObservableCollection<FileItem> SaveItems { get; } = new();

    public event EventHandler<string>? FileOpened;
    public event EventHandler<string>? GameChanged;

    public WorkspaceNavigator()
    {
        InitializeComponent();
        SetupControls();
    }

    private void SetupControls()
    {
        var gameSelector = this.FindControl<ComboBox>("GameSelector");
        var searchBox = this.FindControl<TextBox>("SearchBox");
        var soxTree = this.FindControl<TreeView>("SoxTree");
        var missionsTree = this.FindControl<TreeView>("MissionsTree");
        var savesTree = this.FindControl<TreeView>("SavesTree");

        if (gameSelector != null)
        {
            gameSelector.SelectionChanged += OnGameSelectionChanged;
        }

        if (searchBox != null)
        {
            searchBox.TextChanged += OnSearchTextChanged;
        }

        if (soxTree != null)
        {
            soxTree.ItemsSource = SoxItems;
            soxTree.DoubleTapped += OnItemDoubleTapped;
        }

        if (missionsTree != null)
        {
            missionsTree.ItemsSource = MissionItems;
            missionsTree.DoubleTapped += OnItemDoubleTapped;
        }

        if (savesTree != null)
        {
            savesTree.ItemsSource = SaveItems;
            savesTree.DoubleTapped += OnItemDoubleTapped;
        }
    }

    /// <summary>
    /// Initializes the navigator with settings and loads the default game.
    /// </summary>
    public void Initialize(Settings settings)
    {
        _settings = settings;

        var gameSelector = this.FindControl<ComboBox>("GameSelector");

        // determine which games are available
        var hasCrusaders = !string.IsNullOrEmpty(settings.CrusadersPath);
        var hasHeroes = !string.IsNullOrEmpty(settings.HeroesPath);

        if (gameSelector != null)
        {
            // disable unavailable games
            if (gameSelector.Items[0] is ComboBoxItem crusadersItem)
                crusadersItem.IsEnabled = hasCrusaders;
            if (gameSelector.Items[1] is ComboBoxItem heroesItem)
                heroesItem.IsEnabled = hasHeroes;

            // select first available game
            if (hasCrusaders)
            {
                gameSelector.SelectedIndex = 0;
                LoadGame("Crusaders");
            }
            else if (hasHeroes)
            {
                gameSelector.SelectedIndex = 1;
                LoadGame("Heroes");
            }
        }
    }

    /// <summary>
    /// Gets the currently selected game name.
    /// </summary>
    public string? CurrentGame => _currentGame;

    private void OnGameSelectionChanged(object? sender, SelectionChangedEventArgs e)
    {
        if (_settings == null) return;

        var combo = sender as ComboBox;
        var selected = combo?.SelectedItem as ComboBoxItem;

        if (selected == null) return;

        // Get the game name from the ComboBoxItem content
        var gameName = selected.Content?.ToString();
        if (!string.IsNullOrEmpty(gameName))
        {
            LoadGame(gameName);
        }
    }

    private void LoadGame(string gameName)
    {
        _currentGame = gameName;

        SoxItems.Clear();
        MissionItems.Clear();
        SaveItems.Clear();

        if (_settings == null) return;

        var gamePath = gameName switch
        {
            "Crusaders" => _settings.CrusadersPath,
            "Heroes" => _settings.HeroesPath,
            _ => null
        };

        if (string.IsNullOrEmpty(gamePath)) return;

        LoadSoxFiles(gamePath);
        LoadMissionFiles(gamePath);
        LoadSaveFiles(gameName);

        GameChanged?.Invoke(this, gameName);
    }

    private void LoadSoxFiles(string gamePath)
    {
        var soxPath = Path.Combine(gamePath, "Data", "SOX");
        if (!Directory.Exists(soxPath))
            soxPath = Path.Combine(gamePath, "Data", "Sox");

        if (!Directory.Exists(soxPath)) return;

        try
        {
            // load top-level sox files
            foreach (var file in Directory.GetFiles(soxPath, "*.sox").OrderBy(f => Path.GetFileName(f)))
            {
                SoxItems.Add(CreateFileItem(file));
            }

            // check for language subdirectories
            foreach (var langDir in Directory.GetDirectories(soxPath).OrderBy(d => Path.GetFileName(d)))
            {
                var langName = Path.GetFileName(langDir);
                var langItem = new FileItem
                {
                    Name = langName,
                    Path = langDir,
                    IsDirectory = true,
                    Category = "Language"
                };

                foreach (var file in Directory.GetFiles(langDir, "*.sox").OrderBy(f => Path.GetFileName(f)))
                {
                    langItem.Children.Add(CreateFileItem(file));
                }

                if (langItem.Children.Count > 0)
                {
                    SoxItems.Add(langItem);
                }
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading SOX files: {ex.Message}");
        }
    }

    private void LoadMissionFiles(string gamePath)
    {
        var missionPath = Path.Combine(gamePath, "Data", "Mission");
        if (!Directory.Exists(missionPath)) return;

        try
        {
            // load .stg files from main mission folder
            foreach (var file in Directory.GetFiles(missionPath, "*.stg").OrderBy(f => Path.GetFileName(f)))
            {
                MissionItems.Add(CreateFileItem(file));
            }

            // check for subdirectories (Briefing, Worldmap, etc.)
            foreach (var subDir in Directory.GetDirectories(missionPath).OrderBy(d => Path.GetFileName(d)))
            {
                var dirName = Path.GetFileName(subDir);
                var dirItem = new FileItem
                {
                    Name = dirName,
                    Path = subDir,
                    IsDirectory = true
                };

                var stgFiles = Directory.GetFiles(subDir, "*.stg");
                var navFiles = Directory.GetFiles(subDir, "*.nav");

                foreach (var file in stgFiles.Concat(navFiles).OrderBy(f => Path.GetFileName(f)))
                {
                    dirItem.Children.Add(CreateFileItem(file));
                }

                if (dirItem.Children.Count > 0)
                {
                    MissionItems.Add(dirItem);
                }
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading mission files: {ex.Message}");
        }
    }

    private void LoadSaveFiles(string gameName)
    {
        var savePath = GetSaveGamePath(gameName);
        if (string.IsNullOrEmpty(savePath) || !Directory.Exists(savePath)) return;

        try
        {
            foreach (var file in Directory.GetFiles(savePath).OrderBy(f => Path.GetFileName(f)))
            {
                SaveItems.Add(CreateFileItem(file));
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading save files: {ex.Message}");
        }
    }

    private static string? GetSaveGamePath(string gameName)
    {
        var documents = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
        var kufFolder = gameName switch
        {
            "Crusaders" => "KUF2 Crusaders",
            "Heroes" => "KUF2 Heroes",
            _ => null
        };

        if (kufFolder == null) return null;

        return Path.Combine(documents, kufFolder);
    }

    private static FileItem CreateFileItem(string path)
    {
        var isDir = Directory.Exists(path);
        var name = Path.GetFileName(path);
        var ext = Path.GetExtension(path).ToLowerInvariant();

        return new FileItem
        {
            Name = string.IsNullOrEmpty(name) ? path : name,
            Path = path,
            IsDirectory = isDir,
            Category = ext switch
            {
                ".sox" => "SOX",
                ".stg" => "Mission",
                ".nav" => "Navigation",
                _ => "Other"
            }
        };
    }

    private void OnSearchTextChanged(object? sender, TextChangedEventArgs e)
    {
        var searchBox = sender as TextBox;
        var query = searchBox?.Text?.ToLowerInvariant() ?? "";

        FilterItems(SoxItems, query);
        FilterItems(MissionItems, query);
        FilterItems(SaveItems, query);
    }

    private static void FilterItems(ObservableCollection<FileItem> items, string query)
    {
        foreach (var item in items)
        {
            item.IsVisible = string.IsNullOrEmpty(query) ||
                             item.Name.Contains(query, StringComparison.CurrentCultureIgnoreCase);

            if (item.Children.Count > 0)
            {
                FilterItems(item.Children, query);
                // show parent if any child matches
                if (!item.IsVisible && item.Children.Any(c => c.IsVisible))
                {
                    item.IsVisible = true;
                }
            }
        }
    }

    private void OnItemDoubleTapped(object? sender, TappedEventArgs e)
    {
        var tree = sender as TreeView;
        var selected = tree?.SelectedItem as FileItem;

        if (selected == null) return;

        if (selected.IsDirectory)
        {
            // toggle expansion handled by TreeView
        }
        else
        {
            FileOpened?.Invoke(this, selected.Path);
        }
    }
}

/// <summary>
/// Represents a file or directory in the workspace navigator.
/// </summary>
public class FileItem
{
    public string Name { get; set; } = string.Empty;
    public string Path { get; set; } = string.Empty;
    public bool IsDirectory { get; set; }
    public string Category { get; set; } = string.Empty;
    public bool IsVisible { get; set; } = true;
    public ObservableCollection<FileItem> Children { get; set; } = new();
}
