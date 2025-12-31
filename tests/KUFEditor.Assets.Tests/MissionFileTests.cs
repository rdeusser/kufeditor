using System.Text;
using Xunit;
using KUFEditor.Assets.Mission;

namespace KUFEditor.Assets.Tests;

public class MissionFileTests : IDisposable
{
    private readonly string _testDir;

    public MissionFileTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"MissionFileTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
        {
            Directory.Delete(_testDir, recursive: true);
        }
    }

    [Fact]
    public void Load_SetsFilePath()
    {
        var filePath = CreateTestMissionFile();

        var mission = MissionFile.Load(filePath);

        Assert.Equal(filePath, mission.FilePath);
    }

    [Fact]
    public void Load_FindsTroopWithValidName()
    {
        var filePath = CreateMissionFileWithTroop("Gerald_01");

        var mission = MissionFile.Load(filePath);

        Assert.Single(mission.Troops);
        Assert.Equal("Gerald_01", mission.Troops[0].InternalName);
    }

    [Fact]
    public void Load_FindsMultipleTroops()
    {
        var filePath = CreateMissionFileWithTroops("Gerald_01", "Regnier_01", "Lucretia_01");

        var mission = MissionFile.Load(filePath);

        Assert.Equal(3, mission.Troops.Count);
    }

    [Fact]
    public void Load_ParsesUniqueId()
    {
        var filePath = CreateMissionFileWithTroop("TestUnit_01", uniqueId: 42);

        var mission = MissionFile.Load(filePath);

        Assert.Single(mission.Troops);
        Assert.Equal(42, mission.Troops[0].UniqueId);
    }

    [Fact]
    public void Load_ParsesCategory()
    {
        var filePath = CreateMissionFileWithTroop("LocalUnit_01", category: UnitCategory.Local);

        var mission = MissionFile.Load(filePath);

        Assert.Single(mission.Troops);
        Assert.Equal(UnitCategory.Local, mission.Troops[0].Category);
    }

    [Fact]
    public void Load_ParsesIsHeroFlag()
    {
        var filePath = CreateMissionFileWithTroop("HeroUnit_01", isHero: true);

        var mission = MissionFile.Load(filePath);

        Assert.Single(mission.Troops);
        Assert.True(mission.Troops[0].IsHero);
    }

    [Fact]
    public void Load_HandlesEmptyFile()
    {
        var filePath = Path.Combine(_testDir, "empty.stg");
        File.WriteAllBytes(filePath, Array.Empty<byte>());

        var mission = MissionFile.Load(filePath);

        Assert.Empty(mission.Troops);
    }

    [Fact]
    public void GetRawBytes_ReturnsFileContents()
    {
        var filePath = Path.Combine(_testDir, "test.stg");
        var content = new byte[] { 0x01, 0x02, 0x03, 0x04 };
        File.WriteAllBytes(filePath, content);

        var bytes = MissionFile.GetRawBytes(filePath);

        Assert.Equal(content, bytes);
    }

    [Fact]
    public void GetHexDump_FormatsCorrectly()
    {
        var filePath = Path.Combine(_testDir, "test.stg");
        var content = new byte[] { 0x48, 0x65, 0x6C, 0x6C, 0x6F }; // "Hello"
        File.WriteAllBytes(filePath, content);

        var hex = MissionFile.GetHexDump(filePath);

        Assert.Contains("00000000", hex);
        Assert.Contains("48 65 6C 6C 6F", hex);
        Assert.Contains("Hello", hex);
    }

    [Fact]
    public void GetHexDump_TruncatesLargeFiles()
    {
        var filePath = Path.Combine(_testDir, "large.stg");
        var content = new byte[5000];
        File.WriteAllBytes(filePath, content);

        var hex = MissionFile.GetHexDump(filePath, maxBytes: 100);

        Assert.Contains("more bytes", hex);
    }

    [Fact]
    public void Save_ThrowsNotImplemented()
    {
        var mission = new MissionData();

        Assert.Throws<NotImplementedException>(() => MissionFile.Save(mission));
    }

    // ===== Data Model Tests =====

    [Fact]
    public void TroopBlock_DefaultValues()
    {
        var troop = new TroopBlock();

        Assert.Equal(string.Empty, troop.InternalName);
        Assert.Equal(0, troop.UniqueId);
        Assert.Equal(UnitCategory.NotUsed, troop.Category);
        Assert.Equal(UnitAllegiance.Ally, troop.Allegiance);
        Assert.False(troop.IsHero);
        Assert.False(troop.IsEnabled);
        Assert.Equal(-1, troop.LeaderHP);
        Assert.Equal(-1, troop.UnitHP);
    }

    [Fact]
    public void UnitTroopData_TotalUnits_Calculates()
    {
        var data = new UnitTroopData
        {
            UnitX = 5,
            UnitY = 4
        };

        Assert.Equal(20, data.TotalUnits);
    }

    [Fact]
    public void CharacterData_HasFourSkillSlots()
    {
        var character = new CharacterData();

        Assert.Equal(4, character.Skills.Length);
    }

    [Fact]
    public void SkillIds_GetName_ReturnsCorrectNames()
    {
        Assert.Equal("Melee", SkillIds.GetName(SkillIds.Melee));
        Assert.Equal("Fire", SkillIds.GetName(SkillIds.Fire));
        Assert.Equal("Lightning", SkillIds.GetName(SkillIds.Lightning));
        Assert.Equal("Curse", SkillIds.GetName(SkillIds.Curse));
    }

    [Fact]
    public void SkillIds_GetName_HandlesUnknown()
    {
        var name = SkillIds.GetName(0xFF);

        Assert.Contains("Unknown", name);
    }

    [Fact]
    public void FlagBearerModels_GetName_ReturnsCorrectNames()
    {
        Assert.Equal("Human", FlagBearerModels.GetName(FlagBearerModels.Human));
        Assert.Equal("Orc", FlagBearerModels.GetName(FlagBearerModels.Orc));
        Assert.Equal("Dark Elf", FlagBearerModels.GetName(FlagBearerModels.DarkElf));
    }

    [Fact]
    public void FlagModels_GetName_ReturnsCorrectNames()
    {
        Assert.Equal("Hironeiden", FlagModels.GetName(FlagModels.Hironeiden));
        Assert.Equal("Hexter", FlagModels.GetName(FlagModels.Hexter));
        Assert.Equal("Vellond", FlagModels.GetName(FlagModels.Vellond));
        Assert.Equal("Ecclesia", FlagModels.GetName(FlagModels.Ecclesia));
    }

    [Fact]
    public void FacingDirections_GetName_ReturnsCardinal()
    {
        Assert.Contains("East", FacingDirections.GetName(0x00));
        Assert.Contains("North", FacingDirections.GetName(0x40));
        Assert.Contains("West", FacingDirections.GetName(0x80));
        Assert.Contains("South", FacingDirections.GetName(0xC0));
    }

    [Fact]
    public void FacingDirections_GetName_ReturnsDegrees()
    {
        var name = FacingDirections.GetName(0x20);

        Assert.Contains("°", name);
    }

    // ===== Helper Methods =====

    private string CreateTestMissionFile()
    {
        var filePath = Path.Combine(_testDir, $"test_{Guid.NewGuid():N}.stg");
        File.WriteAllBytes(filePath, new byte[100]);
        return filePath;
    }

    private string CreateMissionFileWithTroop(
        string name,
        byte uniqueId = 1,
        UnitCategory category = UnitCategory.Local,
        UnitAllegiance allegiance = UnitAllegiance.Ally,
        bool isHero = false,
        bool isEnabled = true)
    {
        var filePath = Path.Combine(_testDir, $"troop_{Guid.NewGuid():N}.stg");

        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms, Encoding.ASCII);

        // Write some padding before the troop block
        writer.Write(new byte[10]);

        // Write 32-byte internal name
        var nameBytes = new byte[32];
        var srcBytes = Encoding.ASCII.GetBytes(name);
        Array.Copy(srcBytes, nameBytes, Math.Min(srcBytes.Length, 32));
        writer.Write(nameBytes);

        // Unique ID
        writer.Write(uniqueId);

        // Category (1 byte for simplicity)
        writer.Write((byte)category);

        // Allegiance
        writer.Write((byte)allegiance);

        // IsHero
        writer.Write((byte)(isHero ? 1 : 0));

        // IsEnabled
        writer.Write((byte)(isEnabled ? 1 : 0));

        // Parser needs at least 64 bytes after name start to parse
        writer.Write(new byte[100]);

        File.WriteAllBytes(filePath, ms.ToArray());
        return filePath;
    }

    private string CreateMissionFileWithTroops(params string[] names)
    {
        var filePath = Path.Combine(_testDir, $"troops_{Guid.NewGuid():N}.stg");

        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms, Encoding.ASCII);

        foreach (var name in names)
        {
            // Some padding between troops (need >64 bytes total after name start)
            writer.Write(new byte[100]);

            // Write 32-byte internal name
            var nameBytes = new byte[32];
            var srcBytes = Encoding.ASCII.GetBytes(name);
            Array.Copy(srcBytes, nameBytes, Math.Min(srcBytes.Length, 32));
            writer.Write(nameBytes);

            // Basic troop data
            writer.Write((byte)1);  // Unique ID
            writer.Write((byte)1);  // Category
            writer.Write((byte)0);  // Allegiance
            writer.Write((byte)0);  // IsHero
            writer.Write((byte)1);  // IsEnabled

            // Padding after troop data (ensure parser has enough bytes)
            writer.Write(new byte[100]);
        }

        // Trailing padding
        writer.Write(new byte[100]);

        File.WriteAllBytes(filePath, ms.ToArray());
        return filePath;
    }
}
