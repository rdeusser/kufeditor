using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Interactivity;

namespace KUFEditor.UI.Views;

public partial class EditorArea : UserControl
{
    public ObservableCollection<EditorTab> Tabs { get; }

    public EditorArea()
    {
        InitializeComponent();
        Tabs = new ObservableCollection<EditorTab>();

        var tabControl = this.FindControl<TabControl>("EditorTabs");
        if (tabControl != null)
        {
            tabControl.ItemsSource = Tabs;
        }
    }

    public void OpenFile(string path)
    {
        // check if already open
        var existing = Tabs.FirstOrDefault(t => t.FilePath == path);
        if (existing != null)
        {
            var tabControl = this.FindControl<TabControl>("EditorTabs");
            if (tabControl != null)
                tabControl.SelectedItem = existing;
            return;
        }

        // create new tab
        var tab = new EditorTab
        {
            Name = Path.GetFileName(path),
            FilePath = path,
            EditorContent = CreateEditor(path)
        };

        Tabs.Add(tab);

        var tabs = this.FindControl<TabControl>("EditorTabs");
        if (tabs != null)
            tabs.SelectedItem = tab;
    }

    private Control CreateEditor(string path)
    {
        var ext = Path.GetExtension(path).ToLowerInvariant();

        // Check if this is a save game file (in Documents/KUF2 folder, no extension)
        if (IsSaveGameFile(path))
        {
            return CreateSaveGameEditor(path);
        }

        // create appropriate editor based on file type
        switch (ext)
        {
            case ".map":
                return CreateMapEditor(path);
            case ".sox":
                return CreateSoxEditor(path);
            case ".stg":
                return CreateMissionEditor(path);
            case ".txt":
            case ".xml":
            case ".json":
                return CreateTextEditor(path);
            default:
                return CreateHexEditor(path);
        }
    }

    private static bool IsSaveGameFile(string path)
    {
        // Save games are in Documents/KUF2 Crusaders or Documents/KUF2 Heroes
        var dir = Path.GetDirectoryName(path) ?? "";
        return dir.Contains("KUF2 Crusaders", StringComparison.OrdinalIgnoreCase) ||
               dir.Contains("KUF2 Heroes", StringComparison.OrdinalIgnoreCase);
    }

    private Control CreateMapEditor(string path)
    {
        var panel = new DockPanel();
        var canvas = new Canvas
        {
            Background = Avalonia.Media.Brushes.DarkGray,
            Width = 1024,
            Height = 768
        };

        panel.Children.Add(new TextBlock
        {
            Text = $"Map Editor: {Path.GetFileName(path)}",
            Margin = new Avalonia.Thickness(10)
        });

        return panel;
    }

    private Control CreateSoxEditor(string path)
    {
        var fileName = Path.GetFileName(path);

        // Check if this is TroopInfo.sox
        if (fileName.Equals("TroopInfo.sox", StringComparison.OrdinalIgnoreCase))
        {
            var editor = new TroopInfoEditor();
            editor.LoadFile(path);
            return editor;
        }

        // Check if this is a text SOX file (in language subdirectory like ENG, FRA, etc.)
        if (IsTextSoxFile(path))
        {
            var editor = new TextSoxEditor();
            editor.LoadFile(path);
            return editor;
        }

        // Default binary SOX editor for other SOX files
        var binaryEditor = new BinarySoxEditor();
        binaryEditor.LoadFile(path);
        return binaryEditor;
    }

    private static bool IsTextSoxFile(string path)
    {
        var dir = Path.GetDirectoryName(path) ?? "";
        var dirName = Path.GetFileName(dir);

        // Text SOX files are in language subdirectories like ENG, FRA, GER, etc.
        var languageDirs = new[] { "ENG", "FRA", "GER", "ITA", "SPA", "KOR", "JPN", "CHT", "CHS" };
        return languageDirs.Contains(dirName, StringComparer.OrdinalIgnoreCase);
    }

    private Control CreateMissionEditor(string path)
    {
        var editor = new MissionEditor();
        editor.LoadFile(path);
        return editor;
    }

    private Control CreateSaveGameEditor(string path)
    {
        var editor = new SaveGameEditor();
        editor.LoadFile(path);
        return editor;
    }

    private Control CreateTextEditor(string path)
    {
        var textBox = new TextBox
        {
            AcceptsReturn = true,
            AcceptsTab = true,
            FontFamily = new Avalonia.Media.FontFamily("Consolas, Courier New, monospace")
        };

        try
        {
            textBox.Text = File.ReadAllText(path);
        }
        catch (Exception ex)
        {
            textBox.Text = $"Error loading file: {ex.Message}";
        }

        return textBox;
    }

    private Control CreateHexEditor(string path)
    {
        var textBox = new TextBox
        {
            IsReadOnly = true,
            FontFamily = new Avalonia.Media.FontFamily("Consolas, Courier New, monospace")
        };

        try
        {
            var bytes = File.ReadAllBytes(path);
            var hex = BitConverter.ToString(bytes.Take(1024).ToArray()).Replace("-", " ");
            textBox.Text = hex;
        }
        catch (Exception ex)
        {
            textBox.Text = $"Error loading file: {ex.Message}";
        }

        return textBox;
    }

    private void OnCloseTab(object? sender, RoutedEventArgs e)
    {
        var button = sender as Button;
        var tab = button?.DataContext as EditorTab;

        if (tab != null)
        {
            Tabs.Remove(tab);
        }
    }

    /// <summary>
    /// Refreshes a file in the editor by reloading its content.
    /// </summary>
    public void RefreshFile(string path)
    {
        var existing = Tabs.FirstOrDefault(t => t.FilePath == path);
        if (existing == null) return;

        // recreate the editor content
        existing.EditorContent = CreateEditor(path);

        // force UI update by removing and re-adding
        var index = Tabs.IndexOf(existing);
        Tabs.Remove(existing);
        Tabs.Insert(index, existing);

        var tabControl = this.FindControl<TabControl>("EditorTabs");
        if (tabControl != null)
            tabControl.SelectedItem = existing;
    }
}

public class EditorTab
{
    public string Name { get; set; } = string.Empty;
    public string FilePath { get; set; } = string.Empty;
    public Control? EditorContent { get; set; }
}