using System;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Avalonia.Controls;
using Avalonia.Interactivity;
using KUFEditor.Assets.TextSox;

namespace KUFEditor.UI.Views;

public partial class TextSoxEditor : UserControl
{
    private TextSoxData? _textSox;
    private string? _filePath;
    private int _searchIndex;

    public TextSoxEditor()
    {
        InitializeComponent();
        SetupControls();
    }

    private void SetupControls()
    {
        var saveButton = this.FindControl<Button>("SaveButton");
        var reloadButton = this.FindControl<Button>("ReloadButton");
        var searchButton = this.FindControl<Button>("SearchButton");
        var searchNextButton = this.FindControl<Button>("SearchNextButton");
        var searchResults = this.FindControl<ListBox>("SearchResults");

        if (saveButton != null) saveButton.Click += OnSave;
        if (reloadButton != null) reloadButton.Click += OnReload;
        if (searchButton != null) searchButton.Click += OnSearch;
        if (searchNextButton != null) searchNextButton.Click += OnSearchNext;
        if (searchResults != null) searchResults.DoubleTapped += OnSearchResultSelected;
    }

    /// <summary>
    /// Loads a text SOX file.
    /// </summary>
    public void LoadFile(string path)
    {
        _filePath = path;

        try
        {
            _textSox = TextSoxFile.Load(path);

            UpdateControl<TextBlock>("FileNameText", tb => tb.Text = Path.GetFileName(path));
            UpdateControl<TextBlock>("EntryCountText", tb => tb.Text = $"{_textSox.Entries.Count} entries");

            // Load entries grid
            var grid = this.FindControl<DataGrid>("EntriesGrid");
            if (grid != null)
                grid.ItemsSource = _textSox.Entries;

            // Load hex view
            LoadHexView();
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading text SOX: {ex.Message}");
            UpdateControl<TextBlock>("FileNameText", tb => tb.Text = $"Error: {ex.Message}");
        }
    }

    private void LoadHexView()
    {
        if (_textSox == null) return;

        var hexDump = TextSoxFile.GetHexDump(_textSox.RawData);
        var hexView = this.FindControl<TextBox>("HexView");
        if (hexView != null)
            hexView.Text = hexDump;
    }

    private void UpdateControl<T>(string name, Action<T> action) where T : Control
    {
        var control = this.FindControl<T>(name);
        if (control != null)
            action(control);
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        if (_textSox == null || string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            TextSoxFile.Save(_textSox, _filePath);
            Console.WriteLine("Text SOX saved successfully");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error saving: {ex.Message}");
        }
    }

    private void OnReload(object? sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrEmpty(_filePath))
            LoadFile(_filePath);
    }

    private void OnSearch(object? sender, RoutedEventArgs e)
    {
        _searchIndex = 0;
        PerformSearch();
    }

    private void OnSearchNext(object? sender, RoutedEventArgs e)
    {
        _searchIndex++;
        PerformSearch();
    }

    private void PerformSearch()
    {
        if (_textSox == null) return;

        var searchBox = this.FindControl<TextBox>("SearchBox");
        var caseSensitive = this.FindControl<CheckBox>("CaseSensitiveCheck");
        var searchResults = this.FindControl<ListBox>("SearchResults");

        if (searchBox == null || searchResults == null) return;

        var query = searchBox.Text ?? "";
        if (string.IsNullOrEmpty(query)) return;

        var comparison = caseSensitive?.IsChecked == true
            ? StringComparison.Ordinal
            : StringComparison.OrdinalIgnoreCase;

        var results = _textSox.Entries
            .Where(e => e.Text.Contains(query, comparison))
            .ToList();

        searchResults.ItemsSource = results;

        // Select in main grid if results found
        if (results.Count > 0)
        {
            var index = _searchIndex % results.Count;
            var entry = results[index];

            var grid = this.FindControl<DataGrid>("EntriesGrid");
            if (grid != null)
            {
                grid.SelectedItem = entry;
                grid.ScrollIntoView(entry, null);
            }
        }
    }

    private void OnSearchResultSelected(object? sender, EventArgs e)
    {
        var listBox = sender as ListBox;
        var entry = listBox?.SelectedItem as TextEntry;

        if (entry == null) return;

        var grid = this.FindControl<DataGrid>("EntriesGrid");
        if (grid != null)
        {
            grid.SelectedItem = entry;
            grid.ScrollIntoView(entry, null);
        }
    }
}
