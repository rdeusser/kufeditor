using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace KUFEditor.Assets.Mission;

/// <summary>
/// Reads and writes mission (.stg) files.
/// </summary>
public static class MissionFile
{
    /// <summary>
    /// Loads a mission file from disk.
    /// </summary>
    public static MissionData Load(string path)
    {
        using var stream = File.OpenRead(path);
        using var reader = new BinaryReader(stream, Encoding.ASCII);

        var mission = new MissionData { FilePath = path };

        // read file to find troop blocks
        // The format is complex and varies, so we scan for recognizable patterns

        // For now, return a minimal structure with the file loaded
        // A full implementation would parse the binary format completely

        try
        {
            var troops = FindTroopBlocks(reader, stream.Length);
            foreach (var troop in troops)
            {
                mission.Troops.Add(troop);
            }
        }
        catch (Exception ex)
        {
            Console.WriteLine($"Error parsing mission file: {ex.Message}");
        }

        return mission;
    }

    /// <summary>
    /// Saves a mission file to disk.
    /// </summary>
    public static void Save(MissionData mission)
    {
        // For now, this is a placeholder
        // A full implementation would rebuild the binary structure
        throw new NotImplementedException("Mission file saving not yet implemented");
    }

    private static List<TroopBlock> FindTroopBlocks(BinaryReader reader, long fileLength)
    {
        var troops = new List<TroopBlock>();

        // Read the raw bytes for analysis
        reader.BaseStream.Position = 0;
        var data = reader.ReadBytes((int)fileLength);

        // Try to find troop count byte and parse from there
        // This is a heuristic approach since the format varies

        // Look for potential troop name patterns (32-byte ASCII strings)
        for (int i = 0; i < data.Length - 64; i++)
        {
            // Check if this looks like a valid internal name (ASCII, reasonable length)
            if (IsValidTroopName(data, i))
            {
                var troop = TryParseTroopBlock(data, i);
                if (troop != null)
                {
                    troops.Add(troop);
                    // Skip ahead to avoid re-parsing
                    i += 63;
                }
            }
        }

        return troops;
    }

    private static bool IsValidTroopName(byte[] data, int offset)
    {
        if (offset + 32 > data.Length) return false;

        // Check first few chars are printable ASCII
        int validChars = 0;
        for (int i = 0; i < 32; i++)
        {
            byte b = data[offset + i];
            if (b == 0) break;
            if (b < 32 || b > 126) return false;
            validChars++;
        }

        // Need at least 3 chars for a valid name
        return validChars >= 3;
    }

    private static TroopBlock? TryParseTroopBlock(byte[] data, int offset)
    {
        try
        {
            // Parse internal name (32 bytes)
            int nameEnd = offset;
            while (nameEnd < offset + 32 && data[nameEnd] != 0)
                nameEnd++;

            var name = Encoding.ASCII.GetString(data, offset, nameEnd - offset);

            // Simple validation - name should start with a letter
            if (string.IsNullOrEmpty(name) || !char.IsLetter(name[0]))
                return null;

            var troop = new TroopBlock
            {
                InternalName = name
            };

            int pos = offset + 32;

            // Unique ID
            if (pos >= data.Length) return troop;
            troop.UniqueId = data[pos++];

            // Try to parse UCD (could be 1 or 4 bytes)
            if (pos >= data.Length) return troop;
            troop.Category = (UnitCategory)(data[pos] & 0x0F);
            pos++;

            // Skip possible padding for 4-byte UCD
            if (pos + 3 < data.Length && data[pos] == 0 && data[pos + 1] == 0 && data[pos + 2] == 0)
            {
                pos += 3;
            }

            // UAD
            if (pos >= data.Length) return troop;
            troop.Allegiance = (UnitAllegiance)(data[pos] & 0x03);
            pos++;

            // IsHero
            if (pos >= data.Length) return troop;
            troop.IsHero = data[pos] != 0;
            pos++;

            // IsEnabled
            if (pos >= data.Length) return troop;
            troop.IsEnabled = data[pos] != 0;
            pos++;

            return troop;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>
    /// Gets the raw bytes of the file for hex viewing.
    /// </summary>
    public static byte[] GetRawBytes(string path)
    {
        return File.ReadAllBytes(path);
    }

    /// <summary>
    /// Creates a hex dump string of the file.
    /// </summary>
    public static string GetHexDump(string path, int maxBytes = 4096)
    {
        var bytes = File.ReadAllBytes(path);
        var length = Math.Min(bytes.Length, maxBytes);
        var sb = new StringBuilder();

        for (int i = 0; i < length; i += 16)
        {
            sb.Append($"{i:X8}  ");

            // Hex part
            for (int j = 0; j < 16; j++)
            {
                if (i + j < length)
                    sb.Append($"{bytes[i + j]:X2} ");
                else
                    sb.Append("   ");

                if (j == 7) sb.Append(" ");
            }

            sb.Append(" |");

            // ASCII part
            for (int j = 0; j < 16 && i + j < length; j++)
            {
                byte b = bytes[i + j];
                sb.Append(b >= 32 && b < 127 ? (char)b : '.');
            }

            sb.AppendLine("|");
        }

        if (bytes.Length > maxBytes)
        {
            sb.AppendLine($"... ({bytes.Length - maxBytes} more bytes)");
        }

        return sb.ToString();
    }
}
