using System.Collections.ObjectModel;

namespace KUFEditor.Assets.Mission;

/// <summary>
/// Represents a complete mission file (.stg).
/// </summary>
public class MissionData
{
    public string FilePath { get; set; } = string.Empty;
    public ObservableCollection<TroopBlock> Troops { get; set; } = new();

    /// <summary>
    /// Raw file bytes for patch-based saving.
    /// </summary>
    public byte[] RawData { get; set; } = Array.Empty<byte>();
}

/// <summary>
/// Represents a single troop block in a mission file.
/// </summary>
public class TroopBlock
{
    // Internal name (32 bytes, ASCII, null-terminated)
    public string InternalName { get; set; } = string.Empty;

    // Unique ID (1 byte) - must be unique in file
    public byte UniqueId { get; set; }

    // Unit Category Data (UCD) - 4 bytes Crusaders, 1 byte Heroes
    public UnitCategory Category { get; set; }

    // Unit Allegiance Data (UAD)
    public UnitAllegiance Allegiance { get; set; }

    // Flags
    public bool IsHero { get; set; }
    public bool IsEnabled { get; set; }

    // HP Overrides (-1 = no override)
    public float LeaderHP { get; set; } = -1;
    public float UnitHP { get; set; } = -1;

    // Leader Data
    public CharacterData Leader { get; set; } = new();

    // Officers (up to 2)
    public CharacterData[] Officers { get; set; } = new CharacterData[2] { new(), new() };

    // Unit Troop Data
    public UnitTroopData TroopData { get; set; } = new();

    // Position
    public float PositionX { get; set; }
    public float PositionY { get; set; }
    public byte Facing { get; set; }

    // Flag Visuals
    public byte FlagBearerModel { get; set; }
    public byte FlagModel { get; set; }

    // SP - Skill Points
    public float SkillPoints { get; set; }

    // Extra Stats (optional, 88 bytes = 22 floats)
    public ExtraStats? ExtraStats { get; set; }

    /// <summary>
    /// Byte offsets for patch-based saving.
    /// </summary>
    public TroopOffsets Offsets { get; set; } = new();
}

/// <summary>
/// Tracks byte offsets for each field in a troop block.
/// </summary>
public class TroopOffsets
{
    public int InternalName { get; set; } = -1;
    public int UniqueId { get; set; } = -1;
    public int Category { get; set; } = -1;
    public int CategorySize { get; set; } = 1;
    public int Allegiance { get; set; } = -1;
    public int IsHero { get; set; } = -1;
    public int IsEnabled { get; set; } = -1;
    public int LeaderHP { get; set; } = -1;
    public int UnitHP { get; set; } = -1;
    public int PositionX { get; set; } = -1;
    public int PositionY { get; set; } = -1;
    public int Facing { get; set; } = -1;
    public int SkillPoints { get; set; } = -1;
}

/// <summary>
/// Leader or officer character data.
/// </summary>
public class CharacterData
{
    public byte AnimationId { get; set; }
    public byte ModelId { get; set; }
    public byte WorldmapId { get; set; } = 0xFF;
    public byte Level { get; set; }
    public SkillSlot[] Skills { get; set; } = new SkillSlot[4] { new(), new(), new(), new() };
}

/// <summary>
/// Skill slot with ID and level.
/// </summary>
public class SkillSlot
{
    public byte SkillId { get; set; }
    public byte SkillLevel { get; set; }
}

/// <summary>
/// Unit troop configuration.
/// </summary>
public class UnitTroopData
{
    public byte AnimationId { get; set; }
    public byte ModelId { get; set; }
    public byte UnitX { get; set; }
    public byte UnitY { get; set; }
    public byte TroopInfoId { get; set; }
    public byte FormationId { get; set; }

    /// <summary>
    /// Total units = UnitX × UnitY.
    /// </summary>
    public int TotalUnits => UnitX * UnitY;
}

/// <summary>
/// Optional extra stats block (88 bytes = 22 floats).
/// </summary>
public class ExtraStats
{
    public float MovementSpeed { get; set; }
    public float RotationRate { get; set; }
    public float MoveAcceleration { get; set; }
    public float MoveDeceleration { get; set; }
    public float SightRange { get; set; }
    public float AttackRangeMax { get; set; }
    public float AttackRangeMin { get; set; }
    public float DirectDamage { get; set; }
    public float IndirectDamage { get; set; }
    public float Defense { get; set; }
    public float Width { get; set; }
    public float[] Resistances { get; set; } = new float[11];
}

/// <summary>
/// Unit category data (UCD).
/// </summary>
public enum UnitCategory : byte
{
    NotUsed = 0x00,
    Local = 0x01,
    Remote = 0x02,
    AiEnemy = 0x03,
    AiFriendly = 0x04,
    AiNeutral = 0x05
}

/// <summary>
/// Unit allegiance data (UAD).
/// </summary>
public enum UnitAllegiance : byte
{
    Ally = 0x00,
    Enemy = 0x01,
    EnemyOfEveryone = 0x02
}

/// <summary>
/// Standard skill IDs.
/// </summary>
public static class SkillIds
{
    public const byte Melee = 0x00;
    public const byte Range = 0x01;
    public const byte Frontal = 0x02;
    public const byte Riding = 0x03;
    public const byte Teamwork = 0x04;
    public const byte Scouting = 0x05;
    public const byte Gunpowder = 0x06;
    public const byte Taming = 0x07;
    public const byte Fire = 0x08;
    public const byte Ice = 0x09;
    public const byte Lightning = 0x0A;
    public const byte Holy = 0x0B;
    public const byte Earth = 0x0C;
    public const byte Curse = 0x0D;

    public static string GetName(byte id) => id switch
    {
        Melee => "Melee",
        Range => "Range",
        Frontal => "Frontal",
        Riding => "Riding",
        Teamwork => "Teamwork",
        Scouting => "Scouting",
        Gunpowder => "Gunpowder",
        Taming => "Taming",
        Fire => "Fire",
        Ice => "Ice",
        Lightning => "Lightning",
        Holy => "Holy",
        Earth => "Earth",
        Curse => "Curse",
        _ => $"Unknown ({id:X2})"
    };
}

/// <summary>
/// Flag bearer models by race.
/// </summary>
public static class FlagBearerModels
{
    public const byte Human = 0x00;
    public const byte Orc = 0x01;
    public const byte DarkElf = 0x02;

    public static string GetName(byte id) => id switch
    {
        Human => "Human",
        Orc => "Orc",
        DarkElf => "Dark Elf",
        _ => $"Unknown ({id:X2})"
    };
}

/// <summary>
/// Flag models by faction.
/// </summary>
public static class FlagModels
{
    public const byte Hironeiden = 0x00;
    public const byte Hexter = 0x01;
    public const byte Vellond = 0x02;
    public const byte Ecclesia = 0x03;

    public static string GetName(byte id) => id switch
    {
        Hironeiden => "Hironeiden",
        Hexter => "Hexter",
        Vellond => "Vellond",
        Ecclesia => "Ecclesia",
        _ => $"Unknown ({id:X2})"
    };
}

/// <summary>
/// Facing direction (counter-clockwise from right).
/// </summary>
public static class FacingDirections
{
    public static string GetName(byte facing)
    {
        return facing switch
        {
            0x00 => "East (Right)",
            0x40 => "North (Up)",
            0x80 => "West (Left)",
            0xC0 => "South (Down)",
            _ => $"{facing * 360 / 256}°"
        };
    }
}
