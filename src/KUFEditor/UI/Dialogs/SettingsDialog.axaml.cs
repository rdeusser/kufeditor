using System;
using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using KUFEditor.Core;

namespace KUFEditor.UI.Dialogs;

public partial class SettingsDialog : Window
{
    public Settings Settings { get; private set; }

    public SettingsDialog() : this(new Settings())
    {
    }

    public SettingsDialog(Settings settings)
    {
        InitializeComponent();
        Settings = settings.Clone();
        SetupControls();
        LoadSettings();
    }

    private void SetupControls()
    {
        var closeButton = this.FindControl<Button>("CloseButton");
        var cancelButton = this.FindControl<Button>("CancelButton");
        var saveButton = this.FindControl<Button>("SaveButton");
        var browseCrusadersButton = this.FindControl<Button>("BrowseCrusadersButton");
        var browseHeroesButton = this.FindControl<Button>("BrowseHeroesButton");
        var browseBackupButton = this.FindControl<Button>("BrowseBackupButton");

        if (closeButton != null) closeButton.Click += OnCancel;
        if (cancelButton != null) cancelButton.Click += OnCancel;
        if (saveButton != null) saveButton.Click += OnSave;
        if (browseCrusadersButton != null) browseCrusadersButton.Click += OnBrowseCrusaders;
        if (browseHeroesButton != null) browseHeroesButton.Click += OnBrowseHeroes;
        if (browseBackupButton != null) browseBackupButton.Click += OnBrowseBackup;
    }

    private void LoadSettings()
    {
        var crusadersPath = this.FindControl<TextBox>("CrusadersPathText");
        var heroesPath = this.FindControl<TextBox>("HeroesPathText");
        var backupDir = this.FindControl<TextBox>("BackupDirText");
        var autoBackup = this.FindControl<CheckBox>("AutoBackupCheck");
        var maxBackups = this.FindControl<NumericUpDown>("MaxBackupsUpDown");
        var confirmExit = this.FindControl<CheckBox>("ConfirmOnExitCheck");
        var restoreSession = this.FindControl<CheckBox>("RestoreLastSessionCheck");

        if (crusadersPath != null) crusadersPath.Text = Settings.CrusadersPath ?? "";
        if (heroesPath != null) heroesPath.Text = Settings.HeroesPath ?? "";
        if (backupDir != null) backupDir.Text = Settings.BackupDirectory ?? Settings.GetDefaultBackupDirectory();
        if (autoBackup != null) autoBackup.IsChecked = Settings.AutoBackup;
        if (maxBackups != null) maxBackups.Value = Settings.MaxBackups;
        if (confirmExit != null) confirmExit.IsChecked = Settings.ConfirmOnExit;
        if (restoreSession != null) restoreSession.IsChecked = Settings.RestoreLastSession;
    }

    private void SaveSettings()
    {
        var crusadersPath = this.FindControl<TextBox>("CrusadersPathText");
        var heroesPath = this.FindControl<TextBox>("HeroesPathText");
        var backupDir = this.FindControl<TextBox>("BackupDirText");
        var autoBackup = this.FindControl<CheckBox>("AutoBackupCheck");
        var maxBackups = this.FindControl<NumericUpDown>("MaxBackupsUpDown");
        var confirmExit = this.FindControl<CheckBox>("ConfirmOnExitCheck");
        var restoreSession = this.FindControl<CheckBox>("RestoreLastSessionCheck");

        Settings.CrusadersPath = crusadersPath?.Text ?? Settings.CrusadersPath;
        Settings.HeroesPath = heroesPath?.Text ?? Settings.HeroesPath;
        Settings.BackupDirectory = backupDir?.Text ?? Settings.BackupDirectory;
        Settings.AutoBackup = autoBackup?.IsChecked ?? Settings.AutoBackup;
        Settings.MaxBackups = (int)(maxBackups?.Value ?? Settings.MaxBackups);
        Settings.ConfirmOnExit = confirmExit?.IsChecked ?? Settings.ConfirmOnExit;
        Settings.RestoreLastSession = restoreSession?.IsChecked ?? Settings.RestoreLastSession;
    }

    private void OnCancel(object? sender, RoutedEventArgs e)
    {
        Close(false);
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        SaveSettings();
        Close(true);
    }

    private async void OnBrowseCrusaders(object? sender, RoutedEventArgs e)
    {
        var folder = await SelectFolder("Select Crusaders Game Directory");
        if (folder != null)
        {
            var textBox = this.FindControl<TextBox>("CrusadersPathText");
            if (textBox != null) textBox.Text = folder;
        }
    }

    private async void OnBrowseHeroes(object? sender, RoutedEventArgs e)
    {
        var folder = await SelectFolder("Select Heroes Game Directory");
        if (folder != null)
        {
            var textBox = this.FindControl<TextBox>("HeroesPathText");
            if (textBox != null) textBox.Text = folder;
        }
    }

    private async void OnBrowseBackup(object? sender, RoutedEventArgs e)
    {
        var folder = await SelectFolder("Select Backup Directory");
        if (folder != null)
        {
            var textBox = this.FindControl<TextBox>("BackupDirText");
            if (textBox != null) textBox.Text = folder;
        }
    }

    private async System.Threading.Tasks.Task<string?> SelectFolder(string title)
    {
        var storage = GetTopLevel(this)?.StorageProvider;
        if (storage == null) return null;

        var result = await storage.OpenFolderPickerAsync(new FolderPickerOpenOptions
        {
            Title = title,
            AllowMultiple = false
        });

        if (result.Count > 0)
        {
            return result[0].Path.LocalPath;
        }

        return null;
    }
}
