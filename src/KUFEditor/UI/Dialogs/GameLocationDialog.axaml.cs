using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Media;
using Avalonia.Platform.Storage;

namespace KUFEditor.UI.Dialogs;

public partial class GameLocationDialog : Window
{
    public string? CrusadersPath { get; private set; }
    public string? HeroesPath { get; private set; }
    public List<string> SoxFiles { get; private set; } = new();

    public GameLocationDialog()
    {
        InitializeComponent();

        // try auto-detect on startup
        AutoDetectPaths();
    }

    private void OnAutoDetect(object? sender, RoutedEventArgs e)
    {
        AutoDetectPaths();
    }

    private void AutoDetectPaths()
    {
        var possiblePaths = GetPossibleGamePaths();

        // check for Crusaders
        foreach (var path in possiblePaths)
        {
            var crusadersPath = Path.Combine(path, "Kingdom Under Fire Crusaders");
            if (Directory.Exists(crusadersPath) && ValidateGamePath(crusadersPath, "Crusaders"))
            {
                SetCrusadersPath(crusadersPath);
                break;
            }

            // also check without space
            crusadersPath = Path.Combine(path, "KingdomUnderFireCrusaders");
            if (Directory.Exists(crusadersPath) && ValidateGamePath(crusadersPath, "Crusaders"))
            {
                SetCrusadersPath(crusadersPath);
                break;
            }
        }

        // check for Heroes
        foreach (var path in possiblePaths)
        {
            var heroesPath = Path.Combine(path, "Kingdom Under Fire Heroes");
            if (Directory.Exists(heroesPath) && ValidateGamePath(heroesPath, "Heroes"))
            {
                SetHeroesPath(heroesPath);
                break;
            }

            // also check without space
            heroesPath = Path.Combine(path, "KingdomUnderFireHeroes");
            if (Directory.Exists(heroesPath) && ValidateGamePath(heroesPath, "Heroes"))
            {
                SetHeroesPath(heroesPath);
                break;
            }
        }
    }

    private List<string> GetPossibleGamePaths()
    {
        var paths = new List<string>();

        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            // steam paths on Windows
            paths.Add(@"C:\Program Files (x86)\Steam\steamapps\common");
            paths.Add(@"C:\Program Files\Steam\steamapps\common");
            paths.Add(@"D:\Steam\steamapps\common");
            paths.Add(@"D:\SteamLibrary\steamapps\common");

            // gOG paths
            paths.Add(@"C:\Program Files (x86)\GOG Galaxy\Games");
            paths.Add(@"C:\GOG Games");
        }
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
        {
            // steam paths on macOS
            var home = Environment.GetEnvironmentVariable("HOME") ?? "";
            paths.Add(Path.Combine(home, "Library/Application Support/Steam/steamapps/common"));
            paths.Add("/Applications/Steam.app/Contents/MacOS/steamapps/common");
        }
        else if (RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
        {
            // steam paths on Linux
            var home = Environment.GetEnvironmentVariable("HOME") ?? "";
            paths.Add(Path.Combine(home, ".steam/steam/steamapps/common"));
            paths.Add(Path.Combine(home, ".local/share/Steam/steamapps/common"));
        }

        return paths;
    }

    private bool ValidateGamePath(string path, string gameType)
    {
        // check for Data/SOX directory
        var soxPath = Path.Combine(path, "Data", "SOX");
        if (!Directory.Exists(soxPath))
        {
            soxPath = Path.Combine(path, "Data", "Sox");
            if (!Directory.Exists(soxPath))
            {
                return false;
            }
        }

        // check for at least one SOX file
        var soxFiles = Directory.GetFiles(soxPath, "*.sox", SearchOption.TopDirectoryOnly);
        return soxFiles.Length > 0;
    }

    private void SetCrusadersPath(string path)
    {
        CrusadersPath = path;
        var crusadersPathBox = this.FindControl<TextBox>("CrusadersPathBox");
        var crusadersStatus = this.FindControl<TextBlock>("CrusadersStatus");

        if (crusadersPathBox != null)
            crusadersPathBox.Text = path;

        if (crusadersStatus != null)
        {
            crusadersStatus.Text = "✓ Found";
            crusadersStatus.Foreground = Brushes.Green;
        }

        UpdateSoxFilesList();
        UpdateOkButton();
    }

    private void SetHeroesPath(string path)
    {
        HeroesPath = path;
        var heroesPathBox = this.FindControl<TextBox>("HeroesPathBox");
        var heroesStatus = this.FindControl<TextBlock>("HeroesStatus");

        if (heroesPathBox != null)
            heroesPathBox.Text = path;

        if (heroesStatus != null)
        {
            heroesStatus.Text = "✓ Found";
            heroesStatus.Foreground = Brushes.Green;
        }

        UpdateSoxFilesList();
        UpdateOkButton();
    }

    private void UpdateSoxFilesList()
    {
        SoxFiles.Clear();
        var soxFilesSet = new HashSet<string>();

        if (!string.IsNullOrEmpty(CrusadersPath))
        {
            var soxPath = Path.Combine(CrusadersPath, "Data", "SOX");
            if (!Directory.Exists(soxPath))
                soxPath = Path.Combine(CrusadersPath, "Data", "Sox");

            if (Directory.Exists(soxPath))
            {
                var files = Directory.GetFiles(soxPath, "*.sox", SearchOption.TopDirectoryOnly);
                foreach (var file in files)
                {
                    soxFilesSet.Add(Path.GetFileName(file));
                }
            }
        }

        if (!string.IsNullOrEmpty(HeroesPath))
        {
            var soxPath = Path.Combine(HeroesPath, "Data", "SOX");
            if (!Directory.Exists(soxPath))
                soxPath = Path.Combine(HeroesPath, "Data", "Sox");

            if (Directory.Exists(soxPath))
            {
                var files = Directory.GetFiles(soxPath, "*.sox", SearchOption.TopDirectoryOnly);
                foreach (var file in files)
                {
                    soxFilesSet.Add(Path.GetFileName(file));
                }
            }
        }

        SoxFiles = soxFilesSet.OrderBy(f => f).ToList();

        var soxFilesPanel = this.FindControl<Border>("SoxFilesPanel");
        var soxFilesList = this.FindControl<ItemsControl>("SoxFilesList");

        if (soxFilesPanel != null && SoxFiles.Count > 0)
        {
            soxFilesPanel.IsVisible = true;
            if (soxFilesList != null)
            {
                var displayList = SoxFiles.Select(f => $"  • {f}").ToList();
                displayList.Insert(0, $"Found {SoxFiles.Count} SOX files:");
                soxFilesList.ItemsSource = displayList;
            }
        }
    }

    private void UpdateOkButton()
    {
        var okButton = this.FindControl<Button>("OkButton");
        if (okButton != null)
        {
            okButton.IsEnabled = !string.IsNullOrEmpty(CrusadersPath) || !string.IsNullOrEmpty(HeroesPath);
        }
    }

    private async void OnBrowseCrusaders(object? sender, RoutedEventArgs e)
    {
        var result = await StorageProvider.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Select Kingdom Under Fire: Crusaders Installation Directory",
            AllowMultiple = false
        });

        if (result.Count > 0)
        {
            var path = result[0].Path.LocalPath;
            if (ValidateGamePath(path, "Crusaders"))
            {
                SetCrusadersPath(path);
            }
            else
            {
                var crusadersStatus = this.FindControl<TextBlock>("CrusadersStatus");
                if (crusadersStatus != null)
                {
                    crusadersStatus.Text = "Invalid game directory (Data/SOX not found)";
                    crusadersStatus.Foreground = Brushes.Red;
                }
            }
        }
    }

    private async void OnBrowseHeroes(object? sender, RoutedEventArgs e)
    {
        var result = await StorageProvider.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Select Kingdom Under Fire: Heroes Installation Directory",
            AllowMultiple = false
        });

        if (result.Count > 0)
        {
            var path = result[0].Path.LocalPath;
            if (ValidateGamePath(path, "Heroes"))
            {
                SetHeroesPath(path);
            }
            else
            {
                var heroesStatus = this.FindControl<TextBlock>("HeroesStatus");
                if (heroesStatus != null)
                {
                    heroesStatus.Text = "Invalid game directory (Data/SOX not found)";
                    heroesStatus.Foreground = Brushes.Red;
                }
            }
        }
    }

    private void OnOk(object? sender, RoutedEventArgs e)
    {
        // save paths to settings
        SaveGamePaths();
        Close(true);
    }

    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Close(false);
    }

    private void SaveGamePaths()
    {
        // save to a simple JSON settings file
        var settingsPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "KUFEditor",
            "settings.json"
        );

        var settingsDir = Path.GetDirectoryName(settingsPath);
        if (!string.IsNullOrEmpty(settingsDir) && !Directory.Exists(settingsDir))
        {
            Directory.CreateDirectory(settingsDir);
        }

        var settings = new
        {
            CrusadersPath = CrusadersPath,
            HeroesPath = HeroesPath
        };

        var json = System.Text.Json.JsonSerializer.Serialize(settings, new System.Text.Json.JsonSerializerOptions
        {
            WriteIndented = true
        });

        File.WriteAllText(settingsPath, json);
    }

    public static (string? crusadersPath, string? heroesPath) LoadGamePaths()
    {
        var settingsPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "KUFEditor",
            "settings.json"
        );

        if (File.Exists(settingsPath))
        {
            try
            {
                var json = File.ReadAllText(settingsPath);
                var settings = System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, string>>(json);

                if (settings != null)
                {
                    settings.TryGetValue("CrusadersPath", out var crusadersPath);
                    settings.TryGetValue("HeroesPath", out var heroesPath);
                    return (crusadersPath, heroesPath);
                }
            }
            catch
            {
                // ignore errors reading settings
            }
        }

        return (null, null);
    }
}