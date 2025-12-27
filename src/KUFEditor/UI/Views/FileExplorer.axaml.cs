using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Input;

namespace KUFEditor.UI.Views;

public partial class FileExplorer : UserControl
{
    public ObservableCollection<FileItem> Items { get; }

    public FileExplorer()
    {
        InitializeComponent();
        Items = new ObservableCollection<FileItem>();

        var searchBox = this.FindControl<TextBox>("SearchBox");
        var fileTree = this.FindControl<TreeView>("FileTree");

        if (searchBox != null)
        {
            searchBox.TextChanged += OnSearchTextChanged;
        }

        if (fileTree != null)
        {
            fileTree.ItemsSource = Items;
            fileTree.DoubleTapped += OnItemDoubleTapped;
        }
    }

    public void LoadDirectory(string path)
    {
        Items.Clear();

        if (!Directory.Exists(path))
            return;

        try
        {
            var root = CreateFileItem(path);
            Items.Add(root);
            LoadChildren(root);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading directory: {ex.Message}");
        }
    }

    private void LoadChildren(FileItem parent)
    {
        if (!parent.IsDirectory || parent.Children.Count > 0)
            return;

        try
        {
            var dirInfo = new DirectoryInfo(parent.Path);

            // add directories first
            foreach (var dir in dirInfo.GetDirectories().OrderBy(d => d.Name))
            {
                var item = CreateFileItem(dir.FullName);
                parent.Children.Add(item);
            }

            // then add files
            foreach (var file in dirInfo.GetFiles().OrderBy(f => f.Name))
            {
                var item = CreateFileItem(file.FullName);
                parent.Children.Add(item);
            }
        }
        catch (UnauthorizedAccessException)
        {
            // skip directories we can't access
        }
    }

    private FileItem CreateFileItem(string path)
    {
        var isDir = Directory.Exists(path);
        var name = Path.GetFileName(path);

        if (string.IsNullOrEmpty(name))
            name = path;

        return new FileItem
        {
            Name = name,
            Path = path,
            IsDirectory = isDir,
            Children = new ObservableCollection<FileItem>()
        };
    }

    private void OnSearchTextChanged(object? sender, TextChangedEventArgs e)
    {
        // implement file search
    }

    private void OnItemDoubleTapped(object? sender, TappedEventArgs e)
    {
        var tree = sender as TreeView;
        var selected = tree?.SelectedItem as FileItem;

        if (selected?.IsDirectory == true)
        {
            LoadChildren(selected);
        }
        else if (selected != null)
        {
            FileOpened?.Invoke(this, selected.Path);
        }
    }

    public event EventHandler<string>? FileOpened;
}

public class FileItem
{
    public string Name { get; set; } = string.Empty;
    public string Path { get; set; } = string.Empty;
    public bool IsDirectory { get; set; }
    public ObservableCollection<FileItem> Children { get; set; } = new();
}