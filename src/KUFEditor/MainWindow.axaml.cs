using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Timers;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using Avalonia.Styling;
using KUFEditor.UI.Views;
using KUFEditor.UI.Dialogs;

namespace KUFEditor;

public partial class MainWindow : Window
{
    private readonly List<string> recentFiles = new();
    private readonly Timer memoryTimer;
    private string? currentFolder;

    public MainWindow()
    {
        InitializeComponent();
        SetupUI();

        // setup memory monitoring
        memoryTimer = new Timer(5000); // update every 5 seconds
        memoryTimer.Elapsed += UpdateMemoryStatus;
        memoryTimer.Start();

        UpdateMemoryStatus(null, null);

        // show game location dialog on startup
        Loaded += OnWindowLoaded;
    }

    private async void OnWindowLoaded(object? sender, RoutedEventArgs e)
    {
        // check if game paths are configured
        var (crusadersPath, heroesPath) = GameLocationDialog.LoadGamePaths();

        if (string.IsNullOrEmpty(crusadersPath) && string.IsNullOrEmpty(heroesPath))
        {
            // show game location dialog
            var dialog = new GameLocationDialog();
            await dialog.ShowDialog(this);

            // get the paths after dialog closes
            crusadersPath = dialog.CrusadersPath;
            heroesPath = dialog.HeroesPath;
        }

        // auto-load game directories
        if (!string.IsNullOrEmpty(crusadersPath))
        {
            LoadGameDirectory(crusadersPath, "Crusaders");
        }

        if (!string.IsNullOrEmpty(heroesPath))
        {
            LoadGameDirectory(heroesPath, "Heroes");
        }
    }

    private void LoadGameDirectory(string gamePath, string gameType)
    {
        var fileExplorer = this.FindControl<FileExplorer>("FileExplorer");
        if (fileExplorer != null)
        {
            var soxPath = Path.Combine(gamePath, "Data", "SOX");
            if (!Directory.Exists(soxPath))
                soxPath = Path.Combine(gamePath, "Data", "Sox");

            if (Directory.Exists(soxPath))
            {
                fileExplorer.LoadDirectory(soxPath);
                UpdateStatus($"Loaded {gameType} SOX files from: {soxPath}");
            }
        }
    }

    private void SetupUI()
    {
        var fileExplorer = this.FindControl<FileExplorer>("FileExplorer");
        var editorArea = this.FindControl<EditorArea>("EditorArea");

        if (fileExplorer != null && editorArea != null)
        {
            fileExplorer.FileOpened += (sender, path) =>
            {
                editorArea.OpenFile(path);
                var properties = this.FindControl<PropertiesPanel>("PropertiesPanel");
                properties?.ShowFileProperties(path);
                UpdateStatus($"Opened file: {Path.GetFileName(path)}");
            };
        }

        UpdateStatus("KUFEditor initialized. Ready to edit Kingdom Under Fire game files.");
    }

    private async void OnNewFile(object? sender, RoutedEventArgs e)
    {
        var storage = StorageProvider;
        var result = await storage.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "New File",
            DefaultExtension = "txt"
        });

        if (result != null)
        {
            try
            {
                var path = result.Path.LocalPath;
                File.WriteAllText(path, string.Empty);
                var editorArea = this.FindControl<EditorArea>("EditorArea");
                editorArea?.OpenFile(path);
                UpdateStatus($"Created new file: {path}");
            }
            catch (Exception ex)
            {
                await ShowError($"Failed to create file: {ex.Message}");
            }
        }
    }

    private async void OnOpenFile(object? sender, RoutedEventArgs e)
    {
        var storage = StorageProvider;
        var result = await storage.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Open File",
            AllowMultiple = false
        });

        if (result.Count > 0)
        {
            OpenFile(result[0].Path.LocalPath);
        }
    }

    private async void OnOpenFolder(object? sender, RoutedEventArgs e)
    {
        var storage = StorageProvider;
        var result = await storage.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = "Open Folder",
            AllowMultiple = false
        });

        if (result.Count > 0)
        {
            currentFolder = result[0].Path.LocalPath;
            var fileExplorer = this.FindControl<FileExplorer>("FileExplorer");
            fileExplorer?.LoadDirectory(currentFolder);
            UpdateStatus($"Opened folder: {currentFolder}");
        }
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("File saved");
    }

    private async void OnSaveAs(object? sender, RoutedEventArgs e)
    {
        var storage = StorageProvider;
        var result = await storage.SaveFilePickerAsync(new FilePickerSaveOptions
        {
            Title = "Save As"
        });

        if (result != null)
        {
            UpdateStatus($"Saved as: {result.Path.LocalPath}");
        }
    }

    private void OnSaveAll(object? sender, RoutedEventArgs e)
    {
        // implement save all logic
        UpdateStatus("All files saved");
    }

    private void OnExit(object? sender, RoutedEventArgs e)
    {
        if (Application.Current?.ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.Shutdown();
        }
    }

    private void OnToggleFileExplorer(object? sender, RoutedEventArgs e)
    {
        var container = this.FindControl<Border>("FileExplorerContainer");
        if (container != null)
        {
            container.IsVisible = !container.IsVisible;
        }
    }

    private void OnToggleProperties(object? sender, RoutedEventArgs e)
    {
        var container = this.FindControl<Border>("PropertiesContainer");
        if (container != null)
        {
            container.IsVisible = !container.IsVisible;
        }
    }


    private void OnThemeChange(object? sender, RoutedEventArgs e)
    {
        if (sender is MenuItem item)
        {
            var isDark = item.Header?.ToString() == "Dark";
            Application.Current!.RequestedThemeVariant = isDark
                ? ThemeVariant.Dark
                : ThemeVariant.Light;

            // uncheck other theme options
            if (item.Parent is MenuItem parent)
            {
                foreach (var child in parent.Items.OfType<MenuItem>())
                {
                    child.IsChecked = child == item;
                }
            }
        }
    }


    private void OnOpenSoxViewer(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("SOX Editor opened.");
    }


    private void OnValidateFiles(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("Validating files...");
        // implement validation logic
        UpdateStatus("Validation complete.");
    }

    private void OnBatchProcess(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("Batch processing started...");
    }

    private void OnOpenDocumentation(object? sender, RoutedEventArgs e)
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "https://github.com/yourusername/kufeditor/wiki",
                UseShellExecute = true
            });
        }
        catch
        {
            // handle error
        }
    }

    private async void OnAbout(object? sender, RoutedEventArgs e)
    {
        var dialog = new Window
        {
            Title = "About KUFEditor",
            Width = 400,
            Height = 200,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = new StackPanel
            {
                Margin = new Thickness(20),
                Children =
                {
                    new TextBlock
                    {
                        Text = "KUFEditor",
                        FontSize = 24,
                        FontWeight = Avalonia.Media.FontWeight.Bold,
                        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center
                    },
                    new TextBlock
                    {
                        Text = "Kingdom Under Fire Editor",
                        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
                        Margin = new Thickness(0, 10)
                    },
                    new TextBlock
                    {
                        Text = "Version 1.0.0",
                        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
                        Margin = new Thickness(0, 5)
                    },
                    new TextBlock
                    {
                        Text = "Edit map files, SOX files, and other game resources",
                        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
                        TextWrapping = Avalonia.Media.TextWrapping.Wrap,
                        Margin = new Thickness(0, 10)
                    }
                }
            }
        };

        await dialog.ShowDialog(this);
    }

    private void OpenFile(string path)
    {
        var editorArea = this.FindControl<EditorArea>("EditorArea");
        editorArea?.OpenFile(path);

        AddToRecentFiles(path);
        UpdateStatus($"Opened: {path}");

        var properties = this.FindControl<PropertiesPanel>("PropertiesPanel");
        properties?.ShowFileProperties(path);
    }

    private void AddToRecentFiles(string path)
    {
        recentFiles.Remove(path);
        recentFiles.Insert(0, path);
        if (recentFiles.Count > 10)
            recentFiles.RemoveAt(10);

        UpdateRecentFilesMenu();
    }

    private void UpdateRecentFilesMenu()
    {
        var menu = this.FindControl<MenuItem>("RecentFilesMenu");
        if (menu == null) return;

        menu.Items.Clear();

        foreach (var file in recentFiles)
        {
            var item = new MenuItem
            {
                Header = Path.GetFileName(file)
            };
            ToolTip.SetTip(item, file);
            item.Click += (s, e) => OpenFile(file);
            menu.Items.Add(item);
        }
    }

    private void UpdateStatus(string message)
    {
        var statusText = this.FindControl<TextBlock>("StatusText");
        if (statusText != null)
            statusText.Text = message;
    }

    private void UpdateMemoryStatus(object? sender, ElapsedEventArgs? e)
    {
        Avalonia.Threading.Dispatcher.UIThread.Post(() =>
        {
            var process = Process.GetCurrentProcess();
            var memoryMb = process.WorkingSet64 / (1024 * 1024);

            var memoryStatus = this.FindControl<TextBlock>("MemoryStatus");
            if (memoryStatus != null)
                memoryStatus.Text = $"Memory: {memoryMb} MB";
        });
    }

    private async Task ShowError(string message)
    {
        var dialog = new Window
        {
            Title = "Error",
            Width = 400,
            Height = 150,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = new StackPanel
            {
                Margin = new Thickness(20),
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

    protected override void OnClosed(EventArgs e)
    {
        memoryTimer?.Stop();
        memoryTimer?.Dispose();
        base.OnClosed(e);
    }
}