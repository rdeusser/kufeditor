using System;
using System.IO;
using System.Text;
using Avalonia.Controls;
using Avalonia.Interactivity;

namespace KUFEditor.UI.Views;

/// <summary>
/// Generic binary SOX file editor with hex view and byte editing.
/// </summary>
public partial class BinarySoxEditor : UserControl
{
    private string? _filePath;
    private byte[]? _data;
    private int _version;
    private int _count;
    private int _recordSize;

    public BinarySoxEditor()
    {
        InitializeComponent();
        SetupControls();
    }

    private void SetupControls()
    {
        var saveButton = this.FindControl<Button>("SaveButton");
        var reloadButton = this.FindControl<Button>("ReloadButton");
        var backupButton = this.FindControl<Button>("BackupButton");
        var setByteButton = this.FindControl<Button>("SetByteButton");
        var goToOffsetButton = this.FindControl<Button>("GoToOffsetButton");
        var goToRecordButton = this.FindControl<Button>("GoToRecordButton");
        var editOffset = this.FindControl<TextBox>("EditOffset");

        if (saveButton != null) saveButton.Click += OnSave;
        if (reloadButton != null) reloadButton.Click += OnReload;
        if (backupButton != null) backupButton.Click += OnBackup;
        if (setByteButton != null) setByteButton.Click += OnSetByte;
        if (goToOffsetButton != null) goToOffsetButton.Click += OnGoToOffset;
        if (goToRecordButton != null) goToRecordButton.Click += OnGoToRecord;
        if (editOffset != null) editOffset.TextChanged += OnOffsetChanged;
    }

    /// <summary>
    /// Loads a binary SOX file.
    /// </summary>
    public void LoadFile(string path)
    {
        _filePath = path;

        try
        {
            _data = File.ReadAllBytes(path);
            ParseHeader();
            UpdateUI();
            LoadHexView();
        }
        catch (Exception ex)
        {
            UpdateControl<TextBlock>("FileNameText", tb => tb.Text = $"Error: {ex.Message}");
        }
    }

    private void ParseHeader()
    {
        if (_data == null || _data.Length < 8)
        {
            _version = 0;
            _count = 0;
            _recordSize = 0;
            return;
        }

        _version = BitConverter.ToInt32(_data, 0);
        _count = BitConverter.ToInt32(_data, 4);

        // Calculate record size from remaining data
        if (_count > 0)
        {
            int dataLength = _data.Length - 8; // subtract header
            _recordSize = dataLength / _count;
        }
    }

    private void UpdateUI()
    {
        UpdateControl<TextBlock>("FileNameText", tb => tb.Text = Path.GetFileName(_filePath ?? ""));
        UpdateControl<TextBlock>("InfoVersion", tb => tb.Text = _version.ToString());
        UpdateControl<TextBlock>("InfoCount", tb => tb.Text = _count.ToString());
        UpdateControl<TextBlock>("InfoRecordSize", tb => tb.Text = $"{_recordSize} bytes");
        UpdateControl<TextBlock>("InfoFileSize", tb => tb.Text = FormatSize(_data?.Length ?? 0));

        // Update record navigator max
        var recordNum = this.FindControl<NumericUpDown>("RecordNumber");
        if (recordNum != null && _count > 0)
        {
            recordNum.Maximum = _count - 1;
        }
    }

    private void LoadHexView()
    {
        if (_data == null) return;

        var hexDump = CreateHexDump(_data);
        UpdateControl<TextBox>("HexView", tb => tb.Text = hexDump);
    }

    private static string CreateHexDump(byte[] data, int maxBytes = 8192)
    {
        var length = Math.Min(data.Length, maxBytes);
        var sb = new StringBuilder();

        for (int i = 0; i < length; i += 16)
        {
            sb.Append($"{i:X8}  ");

            for (int j = 0; j < 16; j++)
            {
                if (i + j < length)
                    sb.Append($"{data[i + j]:X2} ");
                else
                    sb.Append("   ");

                if (j == 7) sb.Append(" ");
            }

            sb.Append(" |");

            for (int j = 0; j < 16 && i + j < length; j++)
            {
                byte b = data[i + j];
                sb.Append(b >= 32 && b < 127 ? (char)b : '.');
            }

            sb.AppendLine("|");
        }

        if (data.Length > maxBytes)
        {
            sb.AppendLine($"... ({data.Length - maxBytes} more bytes)");
        }

        return sb.ToString();
    }

    private void OnSave(object? sender, RoutedEventArgs e)
    {
        if (_data == null || string.IsNullOrEmpty(_filePath))
            return;

        try
        {
            // Create backup first
            var backupPath = _filePath + ".bak";
            File.Copy(_filePath, backupPath, true);

            File.WriteAllBytes(_filePath, _data);
            Console.WriteLine("File saved successfully");
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
        if (_data == null) return;

        try
        {
            var offsetText = this.FindControl<TextBox>("EditOffset")?.Text ?? "0";
            var valueText = this.FindControl<TextBox>("EditValue")?.Text ?? "00";

            int offset = ParseHexOrDecimal(offsetText);
            byte value = (byte)ParseHexOrDecimal(valueText);

            if (offset >= 0 && offset < _data.Length)
            {
                _data[offset] = value;
                LoadHexView();
                UpdateCurrentValue(offset);
                Console.WriteLine($"Set byte at 0x{offset:X4} to 0x{value:X2}");
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error setting byte: {ex.Message}");
        }
    }

    private void OnGoToOffset(object? sender, RoutedEventArgs e)
    {
        if (_data == null) return;

        try
        {
            var offsetText = this.FindControl<TextBox>("EditOffset")?.Text ?? "0";
            int offset = ParseHexOrDecimal(offsetText);
            UpdateCurrentValue(offset);
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error: {ex.Message}");
        }
    }

    private void OnGoToRecord(object? sender, RoutedEventArgs e)
    {
        if (_data == null || _recordSize <= 0) return;

        var recordNum = this.FindControl<NumericUpDown>("RecordNumber");
        if (recordNum?.Value == null) return;

        int record = (int)recordNum.Value;
        int offset = 8 + (record * _recordSize); // 8 bytes for header

        UpdateControl<TextBlock>("RecordOffset", tb => tb.Text = $"0x{offset:X4}");
        UpdateControl<TextBox>("EditOffset", tb => tb.Text = $"0x{offset:X4}");
        UpdateCurrentValue(offset);
    }

    private void OnOffsetChanged(object? sender, TextChangedEventArgs e)
    {
        if (_data == null) return;

        try
        {
            var offsetText = (sender as TextBox)?.Text ?? "0";
            int offset = ParseHexOrDecimal(offsetText);
            UpdateCurrentValue(offset);
        }
        catch
        {
            UpdateControl<TextBlock>("CurrentValue", tb => tb.Text = "-");
        }
    }

    private void UpdateCurrentValue(int offset)
    {
        if (_data == null || offset < 0 || offset >= _data.Length)
        {
            UpdateControl<TextBlock>("CurrentValue", tb => tb.Text = "-");
            return;
        }

        byte value = _data[offset];
        UpdateControl<TextBlock>("CurrentValue", tb => tb.Text = $"0x{value:X2} ({value})");
    }

    private static int ParseHexOrDecimal(string text)
    {
        text = text.Trim();
        if (text.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            return Convert.ToInt32(text[2..], 16);
        }
        return int.Parse(text);
    }

    private static string FormatSize(long bytes)
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
}
