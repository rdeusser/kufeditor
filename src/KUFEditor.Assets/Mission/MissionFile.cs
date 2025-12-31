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
        var data = File.ReadAllBytes(path);
        var mission = new MissionData
        {
            FilePath = path,
            RawData = data
        };

        try
        {
            ParseMission(mission, data);
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
        if (mission.RawData.Length == 0)
            throw new InvalidOperationException("No raw data available");

        var data = (byte[])mission.RawData.Clone();

        foreach (var troop in mission.Troops)
        {
            PatchTroop(data, troop);
        }

        var backupPath = mission.FilePath + ".bak";
        File.Copy(mission.FilePath, backupPath, overwrite: true);

        File.WriteAllBytes(mission.FilePath, data);
        mission.RawData = data;
    }

    private static void ParseMission(MissionData mission, byte[] data)
    {
        // Find troop count - scan for it before the first troop block
        // The troop count byte precedes the first 32-byte ASCII name

        int troopCountOffset = FindTroopCountOffset(data);
        if (troopCountOffset < 0) return;

        int troopCount = data[troopCountOffset];
        int pos = troopCountOffset + 1;

        // Detect game type by parsing first troop and checking UCD size
        bool isCrusaders = DetectCrusadersFormat(data, pos);

        for (int i = 0; i < troopCount && pos < data.Length - 64; i++)
        {
            var troop = ParseTroopBlock(data, ref pos, isCrusaders);
            if (troop != null)
            {
                mission.Troops.Add(troop);
            }
        }
    }

    private static int FindTroopCountOffset(byte[] data)
    {
        // Scan for first valid 32-byte ASCII troop name
        // The byte before it should be the troop count
        for (int i = 1; i < data.Length - 64; i++)
        {
            if (IsValidTroopName(data, i))
            {
                // The byte before should be a reasonable troop count (1-50)
                int count = data[i - 1];
                if (count >= 1 && count <= 50)
                {
                    return i - 1;
                }
            }
        }
        return -1;
    }

    private static bool DetectCrusadersFormat(byte[] data, int troopStart)
    {
        // After name (32) + uniqueId (1), check if next 4 bytes look like 4-byte UCD
        // In Crusaders: UCD is 4 bytes, typically 0x01/0x03 followed by 3 zeros
        int ucdOffset = troopStart + 33;
        if (ucdOffset + 4 > data.Length) return false;

        byte ucd = data[ucdOffset];
        // Check for 3 zero padding bytes after UCD value
        return ucd <= 0x05 &&
               data[ucdOffset + 1] == 0 &&
               data[ucdOffset + 2] == 0 &&
               data[ucdOffset + 3] == 0;
    }

    private static TroopBlock? ParseTroopBlock(byte[] data, ref int pos, bool isCrusaders)
    {
        if (pos + 32 > data.Length) return null;

        var troop = new TroopBlock();
        var o = troop.Offsets;

        // Internal name (32 bytes)
        o.InternalName = pos;
        int nameEnd = pos;
        while (nameEnd < pos + 32 && data[nameEnd] != 0) nameEnd++;
        troop.InternalName = Encoding.ASCII.GetString(data, pos, nameEnd - pos);

        if (string.IsNullOrEmpty(troop.InternalName) || !char.IsLetter(troop.InternalName[0]))
            return null;

        pos += 32;

        // Unique ID (1 byte)
        o.UniqueId = pos;
        troop.UniqueId = data[pos++];

        // UCD (1 or 4 bytes)
        o.Category = pos;
        troop.Category = (UnitCategory)(data[pos] & 0x0F);
        if (isCrusaders)
        {
            o.CategorySize = 4;
            pos += 4;
        }
        else
        {
            o.CategorySize = 1;
            pos += 1;
        }

        // UAD (1 byte)
        o.Allegiance = pos;
        troop.Allegiance = (UnitAllegiance)(data[pos++] & 0x03);

        // IsHero (1 byte)
        o.IsHero = pos;
        troop.IsHero = data[pos++] != 0;

        // IsEnabled (1 byte)
        o.IsEnabled = pos;
        troop.IsEnabled = data[pos++] != 0;

        // HP Overrides (2 floats)
        if (pos + 8 <= data.Length)
        {
            o.LeaderHP = pos;
            troop.LeaderHP = BitConverter.ToSingle(data, pos);
            pos += 4;

            o.UnitHP = pos;
            troop.UnitHP = BitConverter.ToSingle(data, pos);
            pos += 4;
        }

        // Leader Data: AnimationID, ModelID, WorldmapID, Level (4 bytes)
        if (pos + 4 <= data.Length)
        {
            troop.Leader.AnimationId = data[pos++];
            troop.Leader.ModelId = data[pos++];
            troop.Leader.WorldmapId = data[pos++];
            troop.Leader.Level = data[pos++];
        }

        // Leader Skills: 4 slots × 2 bytes (8 bytes)
        if (pos + 8 <= data.Length)
        {
            for (int s = 0; s < 4; s++)
            {
                troop.Leader.Skills[s].SkillId = data[pos++];
                troop.Leader.Skills[s].SkillLevel = data[pos++];
            }
        }

        // Officers: 2 × (4 bytes + 8 bytes skills) = 24 bytes
        for (int off = 0; off < 2 && pos + 12 <= data.Length; off++)
        {
            troop.Officers[off].AnimationId = data[pos++];
            troop.Officers[off].ModelId = data[pos++];
            troop.Officers[off].WorldmapId = data[pos++];
            troop.Officers[off].Level = data[pos++];
            for (int s = 0; s < 4; s++)
            {
                troop.Officers[off].Skills[s].SkillId = data[pos++];
                troop.Officers[off].Skills[s].SkillLevel = data[pos++];
            }
        }

        // Unit Troop Data (6 bytes)
        if (pos + 6 <= data.Length)
        {
            troop.TroopData.AnimationId = data[pos++];
            troop.TroopData.ModelId = data[pos++];
            troop.TroopData.UnitX = data[pos++];
            troop.TroopData.UnitY = data[pos++];
            troop.TroopData.TroopInfoId = data[pos++];
            troop.TroopData.FormationId = data[pos++];
        }

        // Position (8 bytes) + Facing (1 byte)
        if (pos + 9 <= data.Length)
        {
            o.PositionX = pos;
            troop.PositionX = BitConverter.ToSingle(data, pos);
            pos += 4;

            o.PositionY = pos;
            troop.PositionY = BitConverter.ToSingle(data, pos);
            pos += 4;

            o.Facing = pos;
            troop.Facing = data[pos++];
        }

        // Flag Visuals (2 bytes)
        if (pos + 2 <= data.Length)
        {
            troop.FlagBearerModel = data[pos++];
            troop.FlagModel = data[pos++];
        }

        // Skill Points (4 bytes)
        if (pos + 4 <= data.Length)
        {
            o.SkillPoints = pos;
            troop.SkillPoints = BitConverter.ToSingle(data, pos);
            pos += 4;
        }

        return troop;
    }

    private static void PatchTroop(byte[] data, TroopBlock troop)
    {
        var o = troop.Offsets;

        // Patch internal name (32 bytes, null-padded)
        if (o.InternalName >= 0 && o.InternalName + 32 <= data.Length)
        {
            var nameBytes = new byte[32];
            var srcBytes = Encoding.ASCII.GetBytes(troop.InternalName);
            Array.Copy(srcBytes, nameBytes, Math.Min(srcBytes.Length, 31));
            Array.Copy(nameBytes, 0, data, o.InternalName, 32);
        }

        PatchByte(data, o.UniqueId, troop.UniqueId);
        PatchByte(data, o.Category, (byte)troop.Category);
        PatchByte(data, o.Allegiance, (byte)troop.Allegiance);
        PatchByte(data, o.IsHero, (byte)(troop.IsHero ? 1 : 0));
        PatchByte(data, o.IsEnabled, (byte)(troop.IsEnabled ? 1 : 0));
        PatchByte(data, o.Facing, troop.Facing);

        PatchFloat(data, o.LeaderHP, troop.LeaderHP);
        PatchFloat(data, o.UnitHP, troop.UnitHP);
        PatchFloat(data, o.PositionX, troop.PositionX);
        PatchFloat(data, o.PositionY, troop.PositionY);
        PatchFloat(data, o.SkillPoints, troop.SkillPoints);
    }

    private static void PatchByte(byte[] data, int offset, byte value)
    {
        if (offset >= 0 && offset < data.Length)
            data[offset] = value;
    }

    private static void PatchFloat(byte[] data, int offset, float value)
    {
        if (offset >= 0 && offset + 4 <= data.Length)
        {
            var bytes = BitConverter.GetBytes(value);
            Array.Copy(bytes, 0, data, offset, 4);
        }
    }

    private static bool IsValidTroopName(byte[] data, int offset)
    {
        if (offset + 32 > data.Length) return false;

        int validChars = 0;
        for (int i = 0; i < 32; i++)
        {
            byte b = data[offset + i];
            if (b == 0) break;
            if (b < 32 || b > 126) return false;
            validChars++;
        }

        return validChars >= 3;
    }

    /// <summary>
    /// Gets the raw bytes of the file.
    /// </summary>
    public static byte[] GetRawBytes(string path) => File.ReadAllBytes(path);

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

            for (int j = 0; j < 16; j++)
            {
                if (i + j < length)
                    sb.Append($"{bytes[i + j]:X2} ");
                else
                    sb.Append("   ");

                if (j == 7) sb.Append(" ");
            }

            sb.Append(" |");

            for (int j = 0; j < 16 && i + j < length; j++)
            {
                byte b = bytes[i + j];
                sb.Append(b >= 32 && b < 127 ? (char)b : '.');
            }

            sb.AppendLine("|");
        }

        if (bytes.Length > maxBytes)
            sb.AppendLine($"... ({bytes.Length - maxBytes} more bytes)");

        return sb.ToString();
    }
}
