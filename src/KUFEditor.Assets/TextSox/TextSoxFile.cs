using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace KUFEditor.Assets.TextSox;

/// <summary>
/// Reads and writes text SOX files.
/// </summary>
public static class TextSoxFile
{
    /// <summary>
    /// Loads a text SOX file.
    /// </summary>
    public static TextSoxData Load(string path)
    {
        var data = new TextSoxData
        {
            FilePath = path,
            FileName = Path.GetFileName(path)
        };

        data.RawData = File.ReadAllBytes(path);
        ParseEntries(data);

        return data;
    }

    /// <summary>
    /// Saves a text SOX file.
    /// </summary>
    public static void Save(TextSoxData data, string path)
    {
        // Rebuild the raw data from entries
        var newData = BuildRawData(data);
        File.WriteAllBytes(path, newData);
    }

    private static void ParseEntries(TextSoxData data)
    {
        int index = 0;
        int offset = 0;

        while (offset < data.RawData.Length)
        {
            // Read length prefix
            byte maxLength = data.RawData[offset];

            // Validate - length prefix should be reasonable (e.g., 0x0B to 0x40)
            if (maxLength == 0 || maxLength > 0x80)
            {
                // Skip to next byte and try again
                offset++;
                continue;
            }

            // Check if we have enough data
            if (offset + 1 + maxLength > data.RawData.Length)
                break;

            // Read the text
            var textBytes = new byte[maxLength];
            Array.Copy(data.RawData, offset + 1, textBytes, 0, maxLength);

            // Find null terminator
            int textEnd = Array.IndexOf(textBytes, (byte)0);
            if (textEnd < 0) textEnd = maxLength;

            var text = Encoding.GetEncoding(1252).GetString(textBytes, 0, textEnd);

            data.Entries.Add(new TextEntry
            {
                Index = index,
                Offset = offset,
                MaxLength = maxLength,
                Text = text
            });

            offset += 1 + maxLength;
            index++;
        }
    }

    private static byte[] BuildRawData(TextSoxData data)
    {
        using var ms = new MemoryStream();
        var encoding = Encoding.GetEncoding(1252);

        foreach (var entry in data.Entries)
        {
            // Write length prefix
            ms.WriteByte(entry.MaxLength);

            // Prepare text bytes
            var textBytes = new byte[entry.MaxLength];
            var sourceBytes = encoding.GetBytes(entry.Text);

            // Copy text, truncating if necessary
            int copyLen = Math.Min(sourceBytes.Length, entry.MaxLength);
            Array.Copy(sourceBytes, textBytes, copyLen);

            // Remaining bytes are already zero (null padding)
            ms.Write(textBytes, 0, entry.MaxLength);
        }

        return ms.ToArray();
    }

    /// <summary>
    /// Creates a hex dump of the file.
    /// </summary>
    public static string GetHexDump(byte[] data, int maxBytes = 4096)
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
}
