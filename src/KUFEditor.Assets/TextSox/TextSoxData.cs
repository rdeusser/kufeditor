using System.Collections.ObjectModel;

namespace KUFEditor.Assets.TextSox;

/// <summary>
/// Represents a text SOX file with fixed-width text entries.
/// </summary>
public class TextSoxData
{
    public string FilePath { get; set; } = string.Empty;
    public string FileName { get; set; } = string.Empty;
    public ObservableCollection<TextEntry> Entries { get; set; } = new();
    public byte[] RawData { get; set; } = Array.Empty<byte>();
}

/// <summary>
/// Represents a single text entry in a text SOX file.
/// </summary>
public class TextEntry
{
    public int Index { get; set; }
    public int Offset { get; set; }
    public byte MaxLength { get; set; }
    public string Text { get; set; } = string.Empty;

    /// <summary>
    /// Gets the hex offset string.
    /// </summary>
    public string OffsetHex => $"0x{Offset:X4}";

    /// <summary>
    /// Gets the current text length.
    /// </summary>
    public int CurrentLength => Text.Length;

    /// <summary>
    /// Gets whether the text exceeds max length.
    /// </summary>
    public bool IsOverflow => CurrentLength > MaxLength;

    /// <summary>
    /// Gets the remaining characters available.
    /// </summary>
    public int Remaining => MaxLength - CurrentLength;
}
