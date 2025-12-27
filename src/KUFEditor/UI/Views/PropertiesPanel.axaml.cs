using System;
using System.IO;
using Avalonia.Controls;

namespace KUFEditor.UI.Views;

public partial class PropertiesPanel : UserControl
{
    public PropertiesPanel()
    {
        InitializeComponent();
    }

    public void ShowFileProperties(string path)
    {
        var nameBox = this.FindControl<TextBox>("NameProperty");
        var typeText = this.FindControl<TextBlock>("TypeProperty");
        var sizeText = this.FindControl<TextBlock>("SizeProperty");
        var pathText = this.FindControl<TextBlock>("PathProperty");

        if (!File.Exists(path) && !Directory.Exists(path))
        {
            ClearProperties();
            return;
        }

        if (nameBox != null)
            nameBox.Text = Path.GetFileName(path);

        if (pathText != null)
            pathText.Text = path;

        if (File.Exists(path))
        {
            var info = new FileInfo(path);
            if (typeText != null)
                typeText.Text = Path.GetExtension(path);
            if (sizeText != null)
                sizeText.Text = FormatFileSize(info.Length);
        }
        else
        {
            if (typeText != null)
                typeText.Text = "Directory";
            if (sizeText != null)
            {
                try
                {
                    var dirInfo = new DirectoryInfo(path);
                    var fileCount = dirInfo.GetFiles("*", SearchOption.TopDirectoryOnly).Length;
                    var dirCount = dirInfo.GetDirectories("*", SearchOption.TopDirectoryOnly).Length;
                    sizeText.Text = $"{fileCount} files, {dirCount} folders";
                }
                catch
                {
                    sizeText.Text = "N/A";
                }
            }
        }
    }

    public void ShowObjectProperties(object obj)
    {
        // show properties for game objects
        ClearProperties();

        if (obj == null)
            return;

        var advanced = this.FindControl<StackPanel>("AdvancedProperties");
        if (advanced == null)
            return;

        // dynamically add properties based on object type
        var type = obj.GetType();
        foreach (var prop in type.GetProperties())
        {
            var grid = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("120,*"),
                Margin = new Avalonia.Thickness(0, 2)
            };

            var label = new TextBlock
            {
                Text = $"{prop.Name}:",
                VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center
            };
            Grid.SetColumn(label, 0);

            var value = prop.GetValue(obj)?.ToString() ?? "null";
            var valueControl = new TextBox
            {
                Text = value,
                Margin = new Avalonia.Thickness(4, 0)
            };
            Grid.SetColumn(valueControl, 1);

            grid.Children.Add(label);
            grid.Children.Add(valueControl);
            advanced.Children.Add(grid);
        }
    }

    private void ClearProperties()
    {
        var nameBox = this.FindControl<TextBox>("NameProperty");
        var typeText = this.FindControl<TextBlock>("TypeProperty");
        var sizeText = this.FindControl<TextBlock>("SizeProperty");
        var pathText = this.FindControl<TextBlock>("PathProperty");
        var advanced = this.FindControl<StackPanel>("AdvancedProperties");

        if (nameBox != null) nameBox.Text = string.Empty;
        if (typeText != null) typeText.Text = string.Empty;
        if (sizeText != null) sizeText.Text = string.Empty;
        if (pathText != null) pathText.Text = string.Empty;
        if (advanced != null) advanced.Children.Clear();
    }

    private string FormatFileSize(long bytes)
    {
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };
        double len = bytes;
        int order = 0;

        while (len >= 1024 && order < sizes.Length - 1)
        {
            order++;
            len = len / 1024;
        }

        return $"{len:0.##} {sizes[order]}";
    }
}