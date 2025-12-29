using System;
using System.Collections.ObjectModel;
using System.IO;
using Avalonia.Controls;
using Avalonia.Interactivity;
using KUFEditor.Core;

namespace KUFEditor.UI.Views;

public partial class InfoPanel : UserControl
{
    private string? _currentFilePath;
    private string? _currentGame;
    private BackupManager? _backupManager;

    public ObservableCollection<SnapshotInfo> Snapshots { get; } = new();

    public event EventHandler<string>? FileRestored;

    public InfoPanel()
    {
        InitializeComponent();

        var snapshotList = this.FindControl<ListBox>("SnapshotList");
        if (snapshotList != null)
        {
            snapshotList.ItemsSource = Snapshots;
            snapshotList.SelectionChanged += OnSnapshotSelectionChanged;
        }
    }

    /// <summary>
    /// Initializes the panel with the backup manager.
    /// </summary>
    public void Initialize(BackupManager backupManager)
    {
        _backupManager = backupManager;
    }

    /// <summary>
    /// Sets the current game context for snapshot operations.
    /// </summary>
    public void SetCurrentGame(string game)
    {
        _currentGame = game;
        RefreshSnapshots();
    }

    /// <summary>
    /// Shows information for the specified file.
    /// </summary>
    public void ShowFile(string filePath)
    {
        _currentFilePath = filePath;

        var noFileMessage = this.FindControl<TextBlock>("NoFileMessage");
        var fileInfoSection = this.FindControl<StackPanel>("FileInfoSection");
        var snapshotsSection = this.FindControl<StackPanel>("SnapshotsSection");

        if (noFileMessage != null) noFileMessage.IsVisible = false;
        if (fileInfoSection != null) fileInfoSection.IsVisible = true;
        if (snapshotsSection != null) snapshotsSection.IsVisible = true;

        UpdateFileInfo(filePath);
        RefreshSnapshots();
    }

    /// <summary>
    /// Clears the panel when no file is selected.
    /// </summary>
    public void Clear()
    {
        _currentFilePath = null;

        var noFileMessage = this.FindControl<TextBlock>("NoFileMessage");
        var fileInfoSection = this.FindControl<StackPanel>("FileInfoSection");
        var snapshotsSection = this.FindControl<StackPanel>("SnapshotsSection");

        if (noFileMessage != null) noFileMessage.IsVisible = true;
        if (fileInfoSection != null) fileInfoSection.IsVisible = false;
        if (snapshotsSection != null) snapshotsSection.IsVisible = false;

        Snapshots.Clear();
    }

    private void UpdateFileInfo(string filePath)
    {
        var fileName = this.FindControl<TextBlock>("FileName");
        var filePathText = this.FindControl<TextBlock>("FilePath");
        var fileSize = this.FindControl<TextBlock>("FileSize");
        var fileModified = this.FindControl<TextBlock>("FileModified");
        var fileType = this.FindControl<TextBlock>("FileType");

        if (!File.Exists(filePath))
        {
            if (fileName != null) fileName.Text = Path.GetFileName(filePath);
            if (filePathText != null) filePathText.Text = filePath;
            if (fileSize != null) fileSize.Text = "File not found";
            if (fileModified != null) fileModified.Text = "-";
            if (fileType != null) fileType.Text = "-";
            return;
        }

        var info = new FileInfo(filePath);

        if (fileName != null) fileName.Text = info.Name;
        if (filePathText != null) filePathText.Text = info.DirectoryName ?? filePath;
        if (fileSize != null) fileSize.Text = FormatFileSize(info.Length);
        if (fileModified != null) fileModified.Text = info.LastWriteTime.ToString("g");
        if (fileType != null) fileType.Text = GetFileTypeName(info.Extension);
    }

    private void RefreshSnapshots()
    {
        Snapshots.Clear();

        var noSnapshotsMessage = this.FindControl<TextBlock>("NoSnapshotsMessage");
        var restorePristineButton = this.FindControl<Button>("RestorePristineButton");

        if (_backupManager == null || string.IsNullOrEmpty(_currentFilePath) || string.IsNullOrEmpty(_currentGame))
        {
            if (noSnapshotsMessage != null) noSnapshotsMessage.IsVisible = true;
            if (restorePristineButton != null) restorePristineButton.IsEnabled = false;
            return;
        }

        var fileName = Path.GetFileName(_currentFilePath);

        // check for pristine backup
        var hasPristine = _backupManager.HasPristineBackup(_currentGame, fileName);
        if (restorePristineButton != null) restorePristineButton.IsEnabled = hasPristine;

        // load named snapshots
        var snapshots = _backupManager.GetSnapshots(_currentGame, fileName);
        foreach (var snapshot in snapshots)
        {
            Snapshots.Add(snapshot);
        }

        if (noSnapshotsMessage != null) noSnapshotsMessage.IsVisible = Snapshots.Count == 0;
    }

    private void OnSnapshotSelectionChanged(object? sender, SelectionChangedEventArgs e)
    {
        var restoreButton = this.FindControl<Button>("RestoreSnapshotButton");
        var list = sender as ListBox;

        if (restoreButton != null)
        {
            restoreButton.IsEnabled = list?.SelectedItem != null;
        }
    }

    private void OnRestorePristine(object? sender, RoutedEventArgs e)
    {
        if (_backupManager == null || string.IsNullOrEmpty(_currentFilePath) || string.IsNullOrEmpty(_currentGame))
            return;

        var fileName = Path.GetFileName(_currentFilePath);

        try
        {
            _backupManager.RestoreFromPristine(_currentGame, fileName, _currentFilePath);
            UpdateFileInfo(_currentFilePath);
            FileRestored?.Invoke(this, _currentFilePath);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error restoring pristine: {ex.Message}");
        }
    }

    private void OnRestoreSnapshot(object? sender, RoutedEventArgs e)
    {
        var snapshotList = this.FindControl<ListBox>("SnapshotList");
        var selected = snapshotList?.SelectedItem as SnapshotInfo;

        if (_backupManager == null || string.IsNullOrEmpty(_currentFilePath) || selected == null)
            return;

        try
        {
            _backupManager.RestoreSnapshot(selected.Path, _currentFilePath);
            UpdateFileInfo(_currentFilePath);
            FileRestored?.Invoke(this, _currentFilePath);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error restoring snapshot: {ex.Message}");
        }
    }

    private void OnNewSnapshot(object? sender, RoutedEventArgs e)
    {
        if (_backupManager == null || string.IsNullOrEmpty(_currentFilePath) || string.IsNullOrEmpty(_currentGame))
            return;

        // for now, use a timestamp-based name; later we can add a dialog for custom names
        var snapshotName = DateTime.Now.ToString("yyyy-MM-dd HH:mm");

        try
        {
            _backupManager.CreateSnapshot(_currentGame, _currentFilePath, snapshotName);
            RefreshSnapshots();
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error creating snapshot: {ex.Message}");
        }
    }

    private static string FormatFileSize(long bytes)
    {
        if (bytes < 1024) return $"{bytes} B";
        if (bytes < 1024 * 1024) return $"{bytes / 1024.0:F1} KB";
        return $"{bytes / (1024.0 * 1024.0):F1} MB";
    }

    private static string GetFileTypeName(string extension)
    {
        return extension.ToLowerInvariant() switch
        {
            ".sox" => "SOX Data File",
            ".stg" => "Mission File",
            ".nav" => "Navigation File",
            ".txt" => "Text File",
            _ => $"{extension.TrimStart('.').ToUpperInvariant()} File"
        };
    }
}
