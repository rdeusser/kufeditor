using System.Text;
using Xunit;
using KUFEditor.Assets.TroopInfo;

namespace KUFEditor.Assets.Tests;

public class TroopInfoSoxFileTests : IDisposable
{
    private readonly string _testDir;
    private readonly TroopInfoSoxFile _soxFile;

    public TroopInfoSoxFileTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"TroopInfoTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
        _soxFile = new TroopInfoSoxFile();
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
        {
            Directory.Delete(_testDir, recursive: true);
        }
    }

    [Fact]
    public void Extension_ReturnsSox()
    {
        Assert.Equal(".sox", _soxFile.Extension);
    }

    [Fact]
    public void Description_IsNotEmpty()
    {
        Assert.False(string.IsNullOrEmpty(_soxFile.Description));
    }

    [Fact]
    public void CanRead_ReturnsTrueForTroopInfoSox()
    {
        Assert.True(_soxFile.CanRead("TroopInfo.sox"));
        Assert.True(_soxFile.CanRead("troopinfo.sox"));
        Assert.True(_soxFile.CanRead("/some/path/TroopInfo.sox"));
    }

    [Fact]
    public void CanRead_ReturnsFalseForOtherFiles()
    {
        Assert.False(_soxFile.CanRead("SkillInfo.sox"));
        Assert.False(_soxFile.CanRead("TroopInfo.txt"));
        Assert.False(_soxFile.CanRead("OtherFile.sox"));
    }

    [Fact]
    public void CanWrite_MatchesCanRead()
    {
        Assert.True(_soxFile.CanWrite("TroopInfo.sox"));
        Assert.False(_soxFile.CanWrite("SkillInfo.sox"));
    }

    [Fact]
    public void Read_ParsesValidFile()
    {
        var filePath = CreateValidTroopInfoFile();

        var result = _soxFile.Read(filePath);

        Assert.IsType<TroopInfoSox>(result);
        var sox = (TroopInfoSox)result;
        Assert.Equal(TroopInfoSox.VALID_VERSION, sox.Version);
        Assert.Equal(TroopInfoSox.TROOP_COUNT, sox.Count);
        Assert.True(sox.IsValid());
    }

    [Fact]
    public void Read_ParsesTroopData()
    {
        var filePath = CreateValidTroopInfoFile();

        var result = (TroopInfoSox)_soxFile.Read(filePath);

        // Check first troop (index 0)
        var troop = result.TroopInfos[0];
        Assert.Equal(0, troop.Job);
        Assert.Equal(0, troop.TypeID);
        Assert.Equal(1.0f, troop.MoveSpeed);
        Assert.Equal(2.0f, troop.RotateRate);
    }

    [Fact]
    public void Read_ParsesLevelUpData()
    {
        var filePath = CreateValidTroopInfoFile();

        var result = (TroopInfoSox)_soxFile.Read(filePath);
        var troop = result.TroopInfos[0];

        Assert.Equal(3, troop.LevelUpData.Length);
        for (int i = 0; i < 3; i++)
        {
            Assert.Equal(i, troop.LevelUpData[i].SkillID);
            Assert.Equal((float)(i + 1), troop.LevelUpData[i].SkillPerLevel);
        }
    }

    [Fact]
    public void Read_ParsesPadding()
    {
        var filePath = CreateValidTroopInfoFile();

        var result = (TroopInfoSox)_soxFile.Read(filePath);

        Assert.Equal(TroopInfoSox.PADDING_SIZE, result.TheEnd.Length);
    }

    [Fact]
    public void Read_ThrowsOnInvalidVersion()
    {
        var filePath = CreateInvalidVersionFile();

        var ex = Assert.Throws<InvalidDataException>(() => _soxFile.Read(filePath));
        Assert.Contains("Invalid TroopInfo.sox", ex.Message);
    }

    [Fact]
    public void Read_ThrowsOnInvalidCount()
    {
        var filePath = CreateInvalidCountFile();

        var ex = Assert.Throws<InvalidDataException>(() => _soxFile.Read(filePath));
        Assert.Contains("Invalid TroopInfo.sox", ex.Message);
    }

    [Fact]
    public void Write_CreatesValidFile()
    {
        var sox = CreateTestTroopInfoSox();
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");

        _soxFile.Write(filePath, sox);

        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public void Write_CreatesBackupOfExistingFile()
    {
        var sox = CreateTestTroopInfoSox();
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");

        // Create initial file
        _soxFile.Write(filePath, sox);

        // Modify and write again
        sox.TroopInfos[0].MoveSpeed = 999.0f;
        _soxFile.Write(filePath, sox);

        Assert.True(File.Exists(filePath + ".bak"));
    }

    [Fact]
    public void Write_ThrowsOnInvalidDataType()
    {
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");

        Assert.Throws<ArgumentException>(() => _soxFile.Write(filePath, "invalid data"));
    }

    [Fact]
    public void RoundTrip_PreservesData()
    {
        var original = CreateTestTroopInfoSox();
        original.TroopInfos[5].MoveSpeed = 12.34f;
        original.TroopInfos[5].Defense = 56.78f;
        original.TroopInfos[10].ResistFire = 0.5f;
        original.TroopInfos[10].LevelUpData[1].SkillID = 42;
        original.TroopInfos[10].LevelUpData[1].SkillPerLevel = 3.14f;

        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        _soxFile.Write(filePath, original);
        var loaded = (TroopInfoSox)_soxFile.Read(filePath);

        Assert.Equal(original.Version, loaded.Version);
        Assert.Equal(original.Count, loaded.Count);
        Assert.Equal(12.34f, loaded.TroopInfos[5].MoveSpeed);
        Assert.Equal(56.78f, loaded.TroopInfos[5].Defense);
        Assert.Equal(0.5f, loaded.TroopInfos[10].ResistFire);
        Assert.Equal(42, loaded.TroopInfos[10].LevelUpData[1].SkillID);
        Assert.Equal(3.14f, loaded.TroopInfos[10].LevelUpData[1].SkillPerLevel);
    }

    [Fact]
    public void RoundTrip_PreservesAllTroops()
    {
        var original = CreateTestTroopInfoSox();
        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            original.TroopInfos[i].Job = i;
            original.TroopInfos[i].TypeID = i * 2;
        }

        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        _soxFile.Write(filePath, original);
        var loaded = (TroopInfoSox)_soxFile.Read(filePath);

        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            Assert.Equal(i, loaded.TroopInfos[i].Job);
            Assert.Equal(i * 2, loaded.TroopInfos[i].TypeID);
        }
    }

    [Fact]
    public void RestoreFromBackup_RestoresBackup()
    {
        var sox = CreateTestTroopInfoSox();
        sox.TroopInfos[0].MoveSpeed = 100.0f;
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");

        // Create initial file
        _soxFile.Write(filePath, sox);

        // Modify and write (creates backup)
        sox.TroopInfos[0].MoveSpeed = 200.0f;
        _soxFile.Write(filePath, sox);

        // Restore from backup
        TroopInfoSoxFile.RestoreFromBackup(filePath);

        var restored = (TroopInfoSox)_soxFile.Read(filePath);
        Assert.Equal(100.0f, restored.TroopInfos[0].MoveSpeed);
    }

    [Fact]
    public void RestoreFromBackup_ThrowsWhenNoBackup()
    {
        var filePath = Path.Combine(_testDir, "NoBackup.sox");

        Assert.Throws<FileNotFoundException>(() => TroopInfoSoxFile.RestoreFromBackup(filePath));
    }

    // ===== Helper Methods =====

    private string CreateValidTroopInfoFile()
    {
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        using var fs = File.Create(filePath);
        using var writer = new BinaryWriter(fs, Encoding.UTF8, false);

        // Header
        writer.Write(TroopInfoSox.VALID_VERSION); // version = 100
        writer.Write(TroopInfoSox.TROOP_COUNT);   // count = 43

        // Write 43 troops
        for (int t = 0; t < TroopInfoSox.TROOP_COUNT; t++)
        {
            WriteTroopInfo(writer, t);
        }

        // Padding
        writer.Write(new byte[TroopInfoSox.PADDING_SIZE]);

        return filePath;
    }

    private void WriteTroopInfo(BinaryWriter writer, int index)
    {
        writer.Write(index);  // Job
        writer.Write(index);  // TypeID
        writer.Write(1.0f);   // MoveSpeed
        writer.Write(2.0f);   // RotateRate
        writer.Write(3.0f);   // MoveAcceleration
        writer.Write(4.0f);   // MoveDeceleration
        writer.Write(5.0f);   // SightRange
        writer.Write(6.0f);   // AttackRangeMax
        writer.Write(7.0f);   // AttackRangeMin
        writer.Write(8.0f);   // AttackFrontRange
        writer.Write(9.0f);   // DirectAttack
        writer.Write(10.0f);  // IndirectAttack
        writer.Write(11.0f);  // Defense
        writer.Write(12.0f);  // BaseWidth

        // 10 resistances
        for (int i = 0; i < 10; i++)
        {
            writer.Write(1.0f);
        }

        writer.Write(13.0f);  // MaxUnitSpeedMultiplier
        writer.Write(100.0f); // DefaultUnitHP
        writer.Write(0);      // FormationRandom
        writer.Write(5);      // DefaultUnitNumX
        writer.Write(4);      // DefaultUnitNumY
        writer.Write(14.0f);  // UnitHPLevUp

        // 3 LevelUpData entries
        for (int i = 0; i < 3; i++)
        {
            writer.Write(i);           // SkillID
            writer.Write((float)(i + 1)); // SkillPerLevel
        }

        writer.Write(15.0f);  // DamageDistribution
    }

    private string CreateInvalidVersionFile()
    {
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        using var fs = File.Create(filePath);
        using var writer = new BinaryWriter(fs, Encoding.UTF8, false);

        writer.Write(999); // invalid version
        writer.Write(TroopInfoSox.TROOP_COUNT);

        // Write minimal data
        for (int t = 0; t < TroopInfoSox.TROOP_COUNT; t++)
        {
            WriteTroopInfo(writer, t);
        }

        writer.Write(new byte[TroopInfoSox.PADDING_SIZE]);

        return filePath;
    }

    private string CreateInvalidCountFile()
    {
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        using var fs = File.Create(filePath);
        using var writer = new BinaryWriter(fs, Encoding.UTF8, false);

        writer.Write(TroopInfoSox.VALID_VERSION);
        writer.Write(10); // invalid count

        // Write minimal data
        for (int t = 0; t < TroopInfoSox.TROOP_COUNT; t++)
        {
            WriteTroopInfo(writer, t);
        }

        writer.Write(new byte[TroopInfoSox.PADDING_SIZE]);

        return filePath;
    }

    private static TroopInfoSox CreateTestTroopInfoSox()
    {
        var sox = new TroopInfoSox
        {
            Version = TroopInfoSox.VALID_VERSION,
            Count = TroopInfoSox.TROOP_COUNT
        };

        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            sox.TroopInfos[i] = new TroopInfo.TroopInfo
            {
                Job = i,
                TypeID = i,
                MoveSpeed = 1.0f,
                RotateRate = 2.0f,
                MoveAcceleration = 3.0f,
                MoveDeceleration = 4.0f,
                SightRange = 5.0f,
                AttackRangeMax = 6.0f,
                AttackRangeMin = 7.0f,
                AttackFrontRange = 8.0f,
                DirectAttack = 9.0f,
                IndirectAttack = 10.0f,
                Defense = 11.0f,
                BaseWidth = 12.0f,
                ResistMelee = 1.0f,
                ResistRanged = 1.0f,
                ResistFrontal = 1.0f,
                ResistExplosion = 1.0f,
                ResistFire = 1.0f,
                ResistIce = 1.0f,
                ResistLightning = 1.0f,
                ResistHoly = 1.0f,
                ResistCurse = 1.0f,
                ResistPoison = 1.0f,
                MaxUnitSpeedMultiplier = 13.0f,
                DefaultUnitHP = 100.0f,
                FormationRandom = 0,
                DefaultUnitNumX = 5,
                DefaultUnitNumY = 4,
                UnitHPLevUp = 14.0f,
                DamageDistribution = 15.0f
            };

            for (int j = 0; j < 3; j++)
            {
                sox.TroopInfos[i].LevelUpData[j] = new LevelUpData
                {
                    SkillID = j,
                    SkillPerLevel = j + 1.0f
                };
            }
        }

        return sox;
    }
}
