using System.Collections.ObjectModel;

namespace KUFEditor.Assets.SaveGame;

/// <summary>
/// Represents a save game file.
/// </summary>
public class SaveGameData
{
    public string FilePath { get; set; } = string.Empty;
    public string FileName { get; set; } = string.Empty;
    public long FileSize { get; set; }
    public DateTime LastModified { get; set; }
    public ObservableCollection<SaveGameTroop> Troops { get; set; } = new();

    /// <summary>
    /// Raw file bytes for hex editing.
    /// </summary>
    public byte[] RawData { get; set; } = Array.Empty<byte>();
}

/// <summary>
/// Represents a troop in a save game.
/// </summary>
public class SaveGameTroop
{
    public int Index { get; set; }
    public int Offset { get; set; }
    public byte TroopType { get; set; }
    public byte Level { get; set; }

    /// <summary>
    /// Gets the troop type name.
    /// </summary>
    public string TroopTypeName => TroopTypes.GetName(TroopType);

    /// <summary>
    /// Gets the hex offset string.
    /// </summary>
    public string OffsetHex => $"0x{Offset:X4}";
}

/// <summary>
/// Troop type definitions for save games.
/// </summary>
public static class TroopTypes
{
    public static string GetName(byte id) => id switch
    {
        0x00 => "None",
        0x01 => "Infantry",
        0x02 => "Heavy Infantry",
        0x03 => "Longbowmen",
        0x04 => "Spearmen",
        0x05 => "Knights",
        0x06 => "Heavy Cavalry",
        0x07 => "Light Cavalry",
        0x08 => "Scorpion",
        0x09 => "Mortars",
        0x0A => "Sappers",
        0x0B => "Priests",
        0x0C => "Paladins",
        0x0D => "Paladin",
        0x0E => "Axemen",
        0x0F => "Berserkers",
        0x10 => "Orc Infantry",
        0x11 => "Orc Archers",
        0x12 => "Warg Riders",
        0x13 => "Ogres",
        0x14 => "Dark Archers",
        0x15 => "Dark Elves",
        0x16 => "Swamp Mammoth",
        0x17 => "Wyvern",
        0x18 => "Bone Dragon",
        0x19 => "Ghoul",
        0x1A => "Wraith",
        0x1B => "Liche",
        0x1C => "Vampire",
        0x1D => "Zombie",
        0x1E => "Skeleton",
        0x1F => "Giant Spider",
        _ => $"Unknown ({id:X2})"
    };
}

/// <summary>
/// Known offsets for save game data.
/// </summary>
public static class SaveGameOffsets
{
    // Gerald example offsets (may vary by character)
    public const int TroopDataStart = 0x05B0;
    public const int UnitItemId = 0x05B0;
    public const int UnitJob = 0x05C0;
    public const int TroopLevel = 0x05D0;
    public const int SkillData = 0x05E0;
    public const int TroopDataEnd = 0x05F0;

    // Offset between troops (estimated)
    public const int TroopStride = 0x100;
}
