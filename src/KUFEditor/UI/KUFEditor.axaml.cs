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
using KUFEditor.Core;

namespace KUFEditor.UI;

public partial class KUFEditor : Window
{
    private readonly List<string> recentFiles = new();
    private readonly Timer memoryTimer;
    private string? currentFolder;
    private Settings settings;
    private BackupManager? backupManager;
    private bool _infoPanelCollapsed;
    private double _infoPanelWidth = 300;
    private NativeMenuItem? _infoPanelMenuItem;

    public KUFEditor()
    {
        InitializeComponent();

        // load settings
        var settingsPath = Settings.GetDefaultSettingsPath();
        settings = Settings.Load(settingsPath);

        // initialize backup manager
        var backupDir = settings.BackupDirectory;
        if (string.IsNullOrEmpty(backupDir))
        {
            backupDir = Settings.GetDefaultBackupDirectory();
        }
        backupManager = new BackupManager(backupDir);

        SetupNativeMenu();
        SetupUI();

        // setup memory monitoring
        memoryTimer = new Timer(5000);
        memoryTimer.Elapsed += UpdateMemoryStatus;
        memoryTimer.Start();

        UpdateMemoryStatus(null, null);

        // show game location dialog on startup
        Loaded += OnWindowLoaded;
    }

    private void SetupNativeMenu()
    {
        var menu = new NativeMenu();

        // File menu
        var fileMenu = new NativeMenuItem("File");
        var fileSubMenu = new NativeMenu();

        var newItem = new NativeMenuItem("New");
        newItem.Click += (s, e) => OnNewFile(s, new RoutedEventArgs());
        fileSubMenu.Add(newItem);

        var openItem = new NativeMenuItem("Open...");
        openItem.Click += (s, e) => OnOpenFile(s, new RoutedEventArgs());
        fileSubMenu.Add(openItem);

        var openFolderItem = new NativeMenuItem("Open Folder...");
        openFolderItem.Click += (s, e) => OnOpenFolder(s, new RoutedEventArgs());
        fileSubMenu.Add(openFolderItem);

        fileSubMenu.Add(new NativeMenuItemSeparator());

        var saveItem = new NativeMenuItem("Save");
        saveItem.Click += (s, e) => OnSave(s, new RoutedEventArgs());
        fileSubMenu.Add(saveItem);

        var saveAsItem = new NativeMenuItem("Save As...");
        saveAsItem.Click += (s, e) => OnSaveAs(s, new RoutedEventArgs());
        fileSubMenu.Add(saveAsItem);

        fileMenu.Menu = fileSubMenu;
        menu.Add(fileMenu);

        // View menu
        var viewMenu = new NativeMenuItem("View");
        var viewSubMenu = new NativeMenu();

        _infoPanelMenuItem = new NativeMenuItem("Info Panel");
        _infoPanelMenuItem.ToggleType = NativeMenuItemToggleType.CheckBox;
        _infoPanelMenuItem.IsChecked = !_infoPanelCollapsed;
        _infoPanelMenuItem.Click += (s, e) => ToggleInfoPanel();
        viewSubMenu.Add(_infoPanelMenuItem);

        viewMenu.Menu = viewSubMenu;
        menu.Add(viewMenu);

        NativeMenu.SetMenu(this, menu);
    }

    private async void OnWindowLoaded(object? sender, RoutedEventArgs e)
    {
        // check if game paths are configured
        var crusadersPath = settings.CrusadersPath;
        var heroesPath = settings.HeroesPath;

        if (string.IsNullOrEmpty(crusadersPath) && string.IsNullOrEmpty(heroesPath))
        {
            // show game location dialog
            var dialog = new GameLocationDialog();
            await dialog.ShowDialog(this);

            // get the paths after dialog closes
            crusadersPath = dialog.CrusadersPath;
            heroesPath = dialog.HeroesPath;

            // update settings
            settings.CrusadersPath = crusadersPath;
            settings.HeroesPath = heroesPath;
            settings.Save(Settings.GetDefaultSettingsPath());

            // capture pristine backups on first setup
            if (backupManager != null)
            {
                if (!string.IsNullOrEmpty(crusadersPath))
                {
                    UpdateStatus("Capturing pristine backups for Crusaders...");
                    backupManager.CapturePristine(crusadersPath, "Crusaders");
                }
                if (!string.IsNullOrEmpty(heroesPath))
                {
                    UpdateStatus("Capturing pristine backups for Heroes...");
                    backupManager.CapturePristine(heroesPath, "Heroes");
                }
            }
        }

        // initialize workspace navigator
        var navigator = this.FindControl<WorkspaceNavigator>("WorkspaceNavigator");
        if (navigator != null)
        {
            navigator.Initialize(settings);
        }

        // initialize info panel
        var infoPanel = this.FindControl<InfoPanel>("InfoPanel");
        if (infoPanel != null && backupManager != null)
        {
            infoPanel.Initialize(backupManager);
        }

        UpdateStatus("KUFEditor ready.");
    }

    private void SetupUI()
    {
        var navigator = this.FindControl<WorkspaceNavigator>("WorkspaceNavigator");
        var editorArea = this.FindControl<EditorArea>("EditorArea");
        var infoPanel = this.FindControl<InfoPanel>("InfoPanel");

        if (navigator != null && editorArea != null)
        {
            navigator.FileOpened += (sender, path) =>
            {
                editorArea.OpenFile(path);
                infoPanel?.ShowFile(path);
                UpdateStatus($"Opened file: {Path.GetFileName(path)}");
            };

            navigator.GameChanged += (sender, game) =>
            {
                infoPanel?.SetCurrentGame(game);
                UpdateStatus($"Switched to {game}");
            };
        }

        if (infoPanel != null)
        {
            infoPanel.FileRestored += (sender, path) =>
            {
                // reload the file in editor if it's open
                editorArea?.RefreshFile(path);
                UpdateStatus($"Restored file: {Path.GetFileName(path)}");
            };
        }

        // wire up menu items
        SetupMenuItems();

        UpdateStatus("KUFEditor initialized. Ready to edit Kingdom Under Fire game files.");
    }

    private void SetupMenuItems()
    {
        // File menu
        var saveMenuItem = this.FindControl<MenuItem>("SaveMenuItem");
        var saveAllMenuItem = this.FindControl<MenuItem>("SaveAllMenuItem");
        var settingsMenuItem = this.FindControl<MenuItem>("SettingsMenuItem");
        var exitMenuItem = this.FindControl<MenuItem>("ExitMenuItem");

        if (saveMenuItem != null) saveMenuItem.Click += OnSave;
        if (saveAllMenuItem != null) saveAllMenuItem.Click += OnSaveAll;
        if (settingsMenuItem != null) settingsMenuItem.Click += OnSettings;
        if (exitMenuItem != null) exitMenuItem.Click += OnExit;

        // Edit menu
        var undoMenuItem = this.FindControl<MenuItem>("UndoMenuItem");
        var redoMenuItem = this.FindControl<MenuItem>("RedoMenuItem");

        if (undoMenuItem != null) undoMenuItem.Click += OnUndo;
        if (redoMenuItem != null) redoMenuItem.Click += OnRedo;

        // Tools menu
        var backupAllMenuItem = this.FindControl<MenuItem>("BackupAllMenuItem");
        var restoreBackupMenuItem = this.FindControl<MenuItem>("RestoreBackupMenuItem");
        var validateSoxMenuItem = this.FindControl<MenuItem>("ValidateSoxMenuItem");

        if (backupAllMenuItem != null) backupAllMenuItem.Click += OnCreateBackup;
        if (restoreBackupMenuItem != null) restoreBackupMenuItem.Click += OnRestoreBackup;
        if (validateSoxMenuItem != null) validateSoxMenuItem.Click += OnValidateFiles;

        // Help menu
        var aboutMenuItem = this.FindControl<MenuItem>("AboutMenuItem");
        var docsMenuItem = this.FindControl<MenuItem>("DocsMenuItem");
        var gitHubMenuItem = this.FindControl<MenuItem>("GitHubMenuItem");

        if (aboutMenuItem != null) aboutMenuItem.Click += OnAbout;
        if (docsMenuItem != null) docsMenuItem.Click += OnOpenDocumentation;
        if (gitHubMenuItem != null) gitHubMenuItem.Click += OnOpenGitHub;
    }

    private void OnUndo(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("Undo");
    }

    private void OnRedo(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("Redo");
    }

    private async void OnSettings(object? sender, RoutedEventArgs e)
    {
        var dialog = new SettingsDialog(settings);
        var result = await dialog.ShowDialog<bool>(this);

        if (result)
        {
            settings = dialog.Settings;
            settings.Save(Settings.GetDefaultSettingsPath());
            UpdateStatus("Settings saved");
        }
    }

    private async void OnOpenGitHub(object? sender, RoutedEventArgs e)
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "https://github.com/rdeusser/kufeditor",
                UseShellExecute = true
            });
        }
        catch (Exception ex)
        {
            await ShowError($"Failed to open GitHub: {ex.Message}");
        }
    }

    private void ToggleInfoPanel()
    {
        var container = this.FindControl<Border>("InfoPanelContainer");
        var splitter = this.FindControl<GridSplitter>("InfoPanelSplitter");
        var mainGrid = this.FindControl<Grid>("MainGrid");

        if (container == null || mainGrid == null) return;

        _infoPanelCollapsed = !_infoPanelCollapsed;

        // update menu checkmark
        if (_infoPanelMenuItem != null)
        {
            _infoPanelMenuItem.IsChecked = !_infoPanelCollapsed;
        }

        // Column 3 is the splitter, Column 4 is the info panel
        var splitterColumn = mainGrid.ColumnDefinitions[3];
        var infoPanelColumn = mainGrid.ColumnDefinitions[4];

        if (_infoPanelCollapsed)
        {
            container.IsVisible = false;
            if (splitter != null) splitter.IsVisible = false;
            splitterColumn.Width = new GridLength(0);
            infoPanelColumn.Width = new GridLength(0);
        }
        else
        {
            container.IsVisible = true;
            if (splitter != null) splitter.IsVisible = true;
            splitterColumn.Width = new GridLength(8);
            infoPanelColumn.Width = new GridLength(_infoPanelWidth);
        }
    }

    private void OnToggleInfoPanel(object? sender, RoutedEventArgs e)
    {
        ToggleInfoPanel();
    }

    private async void OnNewFile(object? sender, RoutedEventArgs e)
    {
        var dialog = new SaveFileDialog
        {
            Title = "New File",
            DefaultExtension = "txt"
        };

        var result = await dialog.ShowAsync(this);
        if (result != null)
        {
            try
            {
                File.WriteAllText(result, string.Empty);
                var editorArea = this.FindControl<EditorArea>("EditorArea");
                editorArea?.OpenFile(result);
                UpdateStatus($"Created new file: {result}");
            }
            catch (Exception ex)
            {
                await ShowError($"Failed to create file: {ex.Message}");
            }
        }
    }

    private async void OnOpenFile(object? sender, RoutedEventArgs e)
    {
        var dialog = new OpenFileDialog
        {
            Title = "Open File",
            AllowMultiple = false
        };

        var result = await dialog.ShowAsync(this);
        if (result != null && result.Length > 0)
        {
            OpenFile(result[0]);
        }
    }

    private async void OnOpenFolder(object? sender, RoutedEventArgs e)
    {
        var dialog = new OpenFolderDialog
        {
            Title = "Open Folder"
        };

        var result = await dialog.ShowAsync(this);
        if (result != null)
        {
            currentFolder = result;
            UpdateStatus($"Opened folder: {result}");
        }
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("File saved");
    }

    private async void OnSaveAs(object? sender, RoutedEventArgs e)
    {
        var dialog = new SaveFileDialog
        {
            Title = "Save As"
        };

        var result = await dialog.ShowAsync(this);
        if (result != null)
        {
            UpdateStatus($"Saved as: {result}");
        }
    }

    private void OnSaveAll(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("All files saved");
    }

    private void OnExit(object? sender, RoutedEventArgs e)
    {
        if (Application.Current?.ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.Shutdown();
        }
    }

    private void OnToggleWorkspace(object? sender, RoutedEventArgs e)
    {
        var container = this.FindControl<Border>("WorkspaceContainer");
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
        UpdateStatus("Validation complete.");
    }

    private async void OnCreateBackup(object? sender, RoutedEventArgs e)
    {
        if (backupManager == null)
        {
            await ShowError("Backup manager not initialized. Please check settings.");
            return;
        }

        var dialog = new Window
        {
            Title = "Create Backup",
            Width = 400,
            Height = 250,
            WindowStartupLocation = WindowStartupLocation.CenterOwner
        };

        var stackPanel = new StackPanel { Margin = new Thickness(20) };

        stackPanel.Children.Add(new TextBlock
        {
            Text = "Select game to backup:",
            FontWeight = Avalonia.Media.FontWeight.Bold,
            Margin = new Thickness(0, 0, 0, 10)
        });

        var crusadersCheckbox = new CheckBox
        {
            Content = "Kingdom Under Fire: Crusaders",
            IsEnabled = !string.IsNullOrEmpty(settings.CrusadersPath),
            Margin = new Thickness(0, 5)
        };

        var heroesCheckbox = new CheckBox
        {
            Content = "Kingdom Under Fire: Heroes",
            IsEnabled = !string.IsNullOrEmpty(settings.HeroesPath),
            Margin = new Thickness(0, 5)
        };

        stackPanel.Children.Add(crusadersCheckbox);
        stackPanel.Children.Add(heroesCheckbox);

        var statusText = new TextBlock
        {
            Margin = new Thickness(0, 15, 0, 0),
            TextWrapping = Avalonia.Media.TextWrapping.Wrap,
            Foreground = Avalonia.Media.Brushes.Gray
        };
        stackPanel.Children.Add(statusText);

        var buttonPanel = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Margin = new Thickness(0, 15, 0, 0)
        };

        var createButton = new Button
        {
            Content = "Create Backup",
            Margin = new Thickness(0, 0, 10, 0),
            Padding = new Thickness(20, 8)
        };

        var cancelButton = new Button
        {
            Content = "Cancel",
            Padding = new Thickness(20, 8)
        };

        createButton.Click += async (s, args) =>
        {
            try
            {
                var backupCount = 0;

                if (crusadersCheckbox.IsChecked == true && !string.IsNullOrEmpty(settings.CrusadersPath))
                {
                    statusText.Text = "Backing up Crusaders...";
                    var files = backupManager.BackupAllSoxFiles(settings.CrusadersPath, "Crusaders");
                    backupCount += files.Count;
                    backupManager.CleanOldBackups("Crusaders", 10);
                }

                if (heroesCheckbox.IsChecked == true && !string.IsNullOrEmpty(settings.HeroesPath))
                {
                    statusText.Text = "Backing up Heroes...";
                    var files = backupManager.BackupAllSoxFiles(settings.HeroesPath, "Heroes");
                    backupCount += files.Count;
                    backupManager.CleanOldBackups("Heroes", 10);
                }

                if (backupCount > 0)
                {
                    UpdateStatus($"Backup created: {backupCount} files backed up");
                    dialog.Close();
                }
                else
                {
                    statusText.Text = "Please select at least one game to backup.";
                }
            }
            catch (Exception ex)
            {
                statusText.Text = $"Error: {ex.Message}";
            }
        };

        cancelButton.Click += (s, args) => dialog.Close();

        buttonPanel.Children.Add(createButton);
        buttonPanel.Children.Add(cancelButton);
        stackPanel.Children.Add(buttonPanel);

        dialog.Content = stackPanel;
        await dialog.ShowDialog(this);
    }

    private async void OnRestoreBackup(object? sender, RoutedEventArgs e)
    {
        if (backupManager == null)
        {
            await ShowError("Backup manager not initialized. Please check settings.");
            return;
        }

        UpdateStatus("Use the Info Panel to restore files from snapshots.");
    }

    private async void OnBatchProcess(object? sender, RoutedEventArgs e)
    {
        UpdateStatus("Batch processing started...");
    }

    private async void OnOpenDocumentation(object? sender, RoutedEventArgs e)
    {
        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = "https://github.com/rdeusser/kufeditor/wiki",
                UseShellExecute = true
            });
        }
        catch (Exception ex)
        {
            await ShowError($"Failed to open documentation: {ex.Message}");
        }
    }

    private async void OnAbout(object? sender, RoutedEventArgs e)
    {
        var dialog = new AboutDialog();
        await dialog.ShowDialog(this);
    }

    private void OpenFile(string path)
    {
        var editorArea = this.FindControl<EditorArea>("EditorArea");
        editorArea?.OpenFile(path);

        var infoPanel = this.FindControl<InfoPanel>("InfoPanel");
        infoPanel?.ShowFile(path);

        AddToRecentFiles(path);
        UpdateStatus($"Opened: {path}");
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
