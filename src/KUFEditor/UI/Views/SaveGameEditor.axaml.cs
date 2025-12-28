using System;
using System.IO;
using System.Linq;
using System.Text;
using Avalonia.Controls;
using Avalonia.Interactivity;
using KUFEditor.Assets.SaveGame;

namespace KUFEditor.UI.Views;

public partial class SaveGameEditor : UserControl
{
    private SaveGameData? _saveGame;
    private string? _filePath;

    public SaveGameEditor()
    {
        InitializeComponent();
        SetupControls();
        LoadTroopTypeReference();
    }

    private void SetupControls()
    {
        var saveButton = this.FindControl<Button>("SaveButton");
        var reloadButton = this.FindControl<Button>("ReloadButton");
        var backupButton = this.FindControl<Button>("BackupButton");
        var setByteButton = this.FindControl<Button>("SetByteButton");
        var goToOffsetButton = this.FindControl<Button>("GoToOffsetButton");
        var applyTroop1Level = this.FindControl<Button>("ApplyTroop1Level");
        var applyTroop1Type = this.FindControl<Button>("ApplyTroop1Type");

        if (saveButton != null) saveButton.Click += OnSave;
        if (reloadButton != null) reloadButton.Click += OnReload;
        if (backupButton != null) backupButton.Click += OnBackup;
        if (setByteButton != null) setByteButton.Click += OnSetByte;
        if (goToOffsetButton != null) goToOffsetButton.Click += OnGoToOffset;
        if (applyTroop1Level != null) applyTroop1Level.Click += OnApplyTroop1Level;
        if (applyTroop1Type != null) applyTroop1Type.Click += OnApplyTroop1Type;

        // Populate troop type combo
        var troop1Type = this.FindControl<ComboBox>("Troop1Type");
        if (troop1Type != null)
        {
            var types = Enumerable.Range(0, 0x30)
                .Select(i => new { Value = (byte)i, Name = $"0x{i:X2} - {TroopTypes.GetName((byte)i)}" })
                .ToArray();
            troop1Type.ItemsSource = types;
            troop1Type.DisplayMemberBinding = new Avalonia.Data.Binding("Name");
            troop1Type.SelectedValueBinding = new Avalonia.Data.Binding("Value");
            troop1Type.SelectedIndex = 0;
        }
    }

    private void LoadTroopTypeReference()
    {
        var reference = this.FindControl<TextBox>("TroopTypeReference");
        if (reference == null) return;

        var sb = new StringBuilder();
        sb.AppendLine("Common Troop Type IDs:");
        sb.AppendLine("0x00 - None");
        sb.AppendLine("0x01 - Infantry");
        sb.AppendLine("0x02 - Heavy Infantry");
        sb.AppendLine("0x03 - Longbowmen");
        sb.AppendLine("0x04 - Spearmen");
        sb.AppendLine("0x05 - Knights");
        sb.AppendLine("0x06 - Heavy Cavalry");
        sb.AppendLine("0x07 - Light Cavalry");
        sb.AppendLine("0x0D - Paladin");
        sb.AppendLine("0x10 - Orc Infantry");
        sb.AppendLine("0x12 - Warg Riders");
        sb.AppendLine("0x18 - Bone Dragon");
        sb.AppendLine("");
        sb.AppendLine("Known Offsets (Gerald):");
        sb.AppendLine("0x05B0 - Unit Item ID");
        sb.AppendLine("0x05C0 - Unit Job/Type");
        sb.AppendLine("0x05D0 - Troop Level");
        sb.AppendLine("0x05E0 - Skill Data");

        reference.Text = sb.ToString();
    }

    /// <summary>
    /// Loads a save game file.
    /// </summary>
    public void LoadFile(string path)
    {
        _filePath = path;

        try
        {
            _saveGame = SaveGameFile.Load(path);

            UpdateControl<TextBlock>("FileNameText", tb => tb.Text = Path.GetFileName(path));
            UpdateControl<TextBlock>("InfoFileName", tb => tb.Text = _saveGame.FileName);
            UpdateControl<TextBlock>("InfoFileSize", tb => tb.Text = FormatFileSize(_saveGame.FileSize));
            UpdateControl<TextBlock>("InfoLastModified", tb => tb.Text = _saveGame.LastModified.ToString("g"));

            // Load troops grid
            var troopsGrid = this.FindControl<DataGrid>("TroopsGrid");
            if (troopsGrid != null)
                troopsGrid.ItemsSource = _saveGame.Troops;

            // Load hex view
            LoadHexView();

            // Load quick edit values
            LoadQuickEditValues();
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error loading save game: {ex.Message}");
            UpdateControl<TextBlock>("FileNameText", tb => tb.Text = $"Error: {ex.Message}");
        }
    }

    private void LoadHexView()
    {
        if (_saveGame == null) return;

        var hexDump = SaveGameFile.GetHexDump(_saveGame.RawData);
        var hexView = this.FindControl<TextBox>("HexView");
        if (hexView != null)
            hexView.Text = hexDump;
    }

    private void LoadQuickEditValues()
    {
        if (_saveGame == null || _saveGame.RawData.Length < SaveGameOffsets.TroopDataEnd)
            return;

        // Load troop 1 level
        var level = SaveGameFile.GetByte(_saveGame, SaveGameOffsets.TroopLevel);
        UpdateControl<NumericUpDown>("Troop1Level", nud => nud.Value = level);

        // Load troop 1 type
        var type = SaveGameFile.GetByte(_saveGame, SaveGameOffsets.UnitJob);
        var troop1TypeCombo = this.FindControl<ComboBox>("Troop1Type");
        if (troop1TypeCombo != null && type < troop1TypeCombo.Items.Count)
            troop1TypeCombo.SelectedIndex = type;
    }

    private static string FormatFileSize(long bytes)
    {
        if (bytes < 1024) return $"{bytes} bytes";
        if (bytes < 1024 * 1024) return $"{bytes / 1024.0:F1} KB";
        return $"{bytes / (1024.0 * 1024.0):F1} MB";
    }

    private void UpdateControl<T>(string name, Action<T> action) where T : Control
    {
        var control = this.FindControl<T>(name);
        if (control != null)
            action(control);
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        if (_saveGame == null || string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            SaveGameFile.Save(_saveGame, _filePath);
            Console.WriteLine("Save game saved successfully");
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

    private void OnBackup(object? sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_filePath)) return;

        try
        {
            var backupPath = _filePath + $".backup_{DateTime.Now:yyyyMMdd_HHmmss}";
            File.Copy(_filePath, backupPath);
            Console.WriteLine($"Backup created: {backupPath}");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error creating backup: {ex.Message}");
        }
    }

    private void OnSetByte(object? sender, RoutedEventArgs e)
    {
        if (_saveGame == null) return;

        try
        {
            var offsetText = this.FindControl<TextBox>("EditOffset")?.Text ?? "0";
            var valueText = this.FindControl<TextBox>("EditValue")?.Text ?? "00";

            int offset = Convert.ToInt32(offsetText.Replace("0x", ""), 16);
            byte value = Convert.ToByte(valueText.Replace("0x", ""), 16);

            SaveGameFile.SetByte(_saveGame, offset, value);
            LoadHexView();

            Console.WriteLine($"Set byte at 0x{offset:X4} to 0x{value:X2}");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error setting byte: {ex.Message}");
        }
    }

    private void OnGoToOffset(object? sender, RoutedEventArgs e)
    {
        // Scroll hex view to offset
        // For now just log
        var offsetText = this.FindControl<TextBox>("EditOffset")?.Text ?? "0";
        Console.WriteLine($"Go to offset: {offsetText}");
    }

    private void OnApplyTroop1Level(object? sender, RoutedEventArgs e)
    {
        if (_saveGame == null) return;

        var level = (byte)(this.FindControl<NumericUpDown>("Troop1Level")?.Value ?? 1);
        SaveGameFile.SetByte(_saveGame, SaveGameOffsets.TroopLevel, level);
        LoadHexView();

        Console.WriteLine($"Set troop 1 level to {level}");
    }

    private void OnApplyTroop1Type(object? sender, RoutedEventArgs e)
    {
        if (_saveGame == null) return;

        var combo = this.FindControl<ComboBox>("Troop1Type");
        if (combo?.SelectedItem == null) return;

        dynamic item = combo.SelectedItem;
        byte type = (byte)item.Value;

        SaveGameFile.SetByte(_saveGame, SaveGameOffsets.UnitJob, type);
        LoadHexView();

        Console.WriteLine($"Set troop 1 type to 0x{type:X2}");
    }
}
