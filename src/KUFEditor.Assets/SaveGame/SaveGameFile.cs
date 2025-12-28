using System;
using System.IO;
using System.Text;

namespace KUFEditor.Assets.SaveGame;

/// <summary>
/// Reads and writes save game files.
/// </summary>
public static class SaveGameFile
{
    /// <summary>
    /// Loads a save game file.
    /// </summary>
    public static SaveGameData Load(string path)
    {
        var data = new SaveGameData
        {
            FilePath = path,
            FileName = Path.GetFileName(path)
        };

        var fileInfo = new FileInfo(path);
        data.FileSize = fileInfo.Length;
        data.LastModified = fileInfo.LastWriteTime;

        data.RawData = File.ReadAllBytes(path);

        // Try to find troop data
        TryFindTroops(data);

        return data;
    }

    /// <summary>
    /// Saves a save game file.
    /// </summary>
    public static void Save(SaveGameData data, string path)
    {
        File.WriteAllBytes(path, data.RawData);
    }

    private static void TryFindTroops(SaveGameData data)
    {
        if (data.RawData.Length < SaveGameOffsets.TroopDataEnd)
            return;

        // Try to parse troop data at known offsets
        // This is heuristic and may need adjustment per character

        int troopIndex = 0;
        for (int offset = SaveGameOffsets.TroopDataStart;
             offset < data.RawData.Length - 0x40 && troopIndex < 10;
             offset += SaveGameOffsets.TroopStride)
        {
            var troop = TryParseTroop(data.RawData, offset, troopIndex);
            if (troop != null)
            {
                data.Troops.Add(troop);
                troopIndex++;
            }
        }
    }

    private static SaveGameTroop? TryParseTroop(byte[] data, int offset, int index)
    {
        if (offset + 0x30 >= data.Length)
            return null;

        // Try to identify troop type at expected offset
        byte troopType = data[offset + 0x10];

        // Skip if troop type looks invalid
        if (troopType > 0x40)
            return null;

        byte level = data[offset + 0x20];

        // Skip if level looks invalid
        if (level > 99 || level == 0)
            return null;

        return new SaveGameTroop
        {
            Index = index,
            Offset = offset,
            TroopType = troopType,
            Level = level
        };
    }

    /// <summary>
    /// Creates a hex dump of the save game.
    /// </summary>
    public static string GetHexDump(byte[] data, int maxBytes = 8192)
    {
        var length = Math.Min(data.Length, maxBytes);
        var sb = new StringBuilder();

        for (int i = 0; i < length; i += 16)
        {
            sb.Append($"{i:X8}  ");

            // Hex part
            for (int j = 0; j < 16; j++)
            {
                if (i + j < length)
                    sb.Append($"{data[i + j]:X2} ");
                else
                    sb.Append("   ");

                if (j == 7) sb.Append(" ");
            }

            sb.Append(" |");

            // ASCII part
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

    /// <summary>
    /// Modifies a byte at a specific offset.
    /// </summary>
    public static void SetByte(SaveGameData data, int offset, byte value)
    {
        if (offset >= 0 && offset < data.RawData.Length)
        {
            data.RawData[offset] = value;
        }
    }

    /// <summary>
    /// Gets a byte at a specific offset.
    /// </summary>
    public static byte GetByte(SaveGameData data, int offset)
    {
        if (offset >= 0 && offset < data.RawData.Length)
        {
            return data.RawData[offset];
        }
        return 0;
    }
}
