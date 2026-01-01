using System.Text;
using Xunit;
using KUFEditor.Assets.Patching;
using KUFEditor.Assets.TroopInfo;
using KUFEditor.Assets.TextSox;

namespace KUFEditor.Assets.Tests;

public class TroopInfoPatcherTests : IDisposable
{
    private readonly string _testDir;
    private readonly TroopInfoPatcher _patcher;
    private readonly TroopInfoSoxFile _parser;

    public TroopInfoPatcherTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"PatcherTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
        _patcher = new TroopInfoPatcher();
        _parser = new TroopInfoSoxFile();
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
            Directory.Delete(_testDir, recursive: true);
    }

    [Fact]
    public void CanHandle_ReturnsTrueForTroopInfoSox()
    {
        Assert.True(_patcher.CanHandle("TroopInfo.sox"));
        Assert.True(_patcher.CanHandle("troopinfo.sox"));
    }

    [Fact]
    public void CanHandle_ReturnsFalseForOtherFiles()
    {
        Assert.False(_patcher.CanHandle("SkillInfo.sox"));
        Assert.False(_patcher.CanHandle("ItemTypeInfo_ENG.sox"));
    }

    [Fact]
    public void Modify_UpdatesFloatField()
    {
        var filePath = CreateValidTroopInfoFile();

        _patcher.Modify(filePath, "Archer", new Dictionary<string, object>
        {
            { "MoveSpeed", 999.0f }
        });

        var sox = (TroopInfoSox)_parser.Read(filePath);
        Assert.Equal(999.0f, sox.TroopInfos[0].MoveSpeed);
    }

    [Fact]
    public void Modify_UpdatesIntField()
    {
        var filePath = CreateValidTroopInfoFile();

        _patcher.Modify(filePath, "Archer", new Dictionary<string, object>
        {
            { "Job", 42 }
        });

        var sox = (TroopInfoSox)_parser.Read(filePath);
        Assert.Equal(42, sox.TroopInfos[0].Job);
    }

    [Fact]
    public void Modify_UpdatesMultipleFields()
    {
        var filePath = CreateValidTroopInfoFile();

        _patcher.Modify(filePath, "Infantry", new Dictionary<string, object>
        {
            { "MoveSpeed", 10.5f },
            { "Defense", 25.0f },
            { "DirectAttack", 15.0f }
        });

        var sox = (TroopInfoSox)_parser.Read(filePath);
        Assert.Equal(10.5f, sox.TroopInfos[2].MoveSpeed);
        Assert.Equal(25.0f, sox.TroopInfos[2].Defense);
        Assert.Equal(15.0f, sox.TroopInfos[2].DirectAttack);
    }

    [Fact]
    public void Modify_ThrowsOnUnknownTroop()
    {
        var filePath = CreateValidTroopInfoFile();

        var ex = Assert.Throws<InvalidOperationException>(() =>
            _patcher.Modify(filePath, "Unknown Troop", new Dictionary<string, object>
            {
                { "MoveSpeed", 10.0f }
            }));
        Assert.Contains("not found", ex.Message);
    }

    [Fact]
    public void Modify_ThrowsOnUnknownField()
    {
        var filePath = CreateValidTroopInfoFile();

        var ex = Assert.Throws<InvalidOperationException>(() =>
            _patcher.Modify(filePath, "Archer", new Dictionary<string, object>
            {
                { "InvalidField", 10.0f }
            }));
        Assert.Contains("Unknown field", ex.Message);
    }

    [Fact]
    public void Add_ThrowsNotSupported()
    {
        var filePath = CreateValidTroopInfoFile();

        Assert.Throws<NotSupportedException>(() =>
            _patcher.Add(filePath, new Dictionary<string, object>()));
    }

    [Fact]
    public void Delete_ThrowsNotSupported()
    {
        var filePath = CreateValidTroopInfoFile();

        Assert.Throws<NotSupportedException>(() =>
            _patcher.Delete(filePath, "Archer"));
    }

    private string CreateValidTroopInfoFile()
    {
        var filePath = Path.Combine(_testDir, "TroopInfo.sox");
        var sox = CreateTestTroopInfoSox();
        _parser.Write(filePath, sox);

        // Remove the backup created by Write.
        var backupPath = filePath + ".bak";
        if (File.Exists(backupPath))
            File.Delete(backupPath);

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
                Defense = 10.0f,
                DirectAttack = 5.0f
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

public class TextSoxPatcherTests : IDisposable
{
    private readonly string _testDir;
    private readonly TextSoxPatcher _patcher;

    public TextSoxPatcherTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"TextPatcherTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
        _patcher = new TextSoxPatcher();
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
            Directory.Delete(_testDir, recursive: true);
    }

    [Fact]
    public void CanHandle_ReturnsTrueForTextSoxFiles()
    {
        Assert.True(_patcher.CanHandle("ItemTypeInfo_ENG.sox"));
        Assert.True(_patcher.CanHandle("ItemTypeInfo_KOR.sox"));
        Assert.True(_patcher.CanHandle("ItemTypeInfo_JPN.sox"));
    }

    [Fact]
    public void CanHandle_ReturnsFalseForOtherFiles()
    {
        Assert.False(_patcher.CanHandle("TroopInfo.sox"));
        Assert.False(_patcher.CanHandle("SkillInfo.sox"));
    }

    [Fact]
    public void Modify_UpdatesTextEntry()
    {
        var filePath = CreateTestTextSoxFile();

        _patcher.Modify(filePath, "0", new Dictionary<string, object>
        {
            { "Text", "Modified" }
        });

        var data = TextSoxFile.Load(filePath);
        Assert.Equal("Modified", data.Entries[0].Text);
    }

    [Fact]
    public void Modify_ThrowsOnInvalidIndex()
    {
        var filePath = CreateTestTextSoxFile();

        var ex = Assert.Throws<InvalidOperationException>(() =>
            _patcher.Modify(filePath, "abc", new Dictionary<string, object>
            {
                { "Text", "Test" }
            }));
        Assert.Contains("Invalid record name", ex.Message);
    }

    [Fact]
    public void Modify_ThrowsOnTextTooLong()
    {
        var filePath = CreateTestTextSoxFile();

        var ex = Assert.Throws<InvalidOperationException>(() =>
            _patcher.Modify(filePath, "0", new Dictionary<string, object>
            {
                { "Text", new string('x', 100) }
            }));
        Assert.Contains("exceeds max length", ex.Message);
    }

    [Fact]
    public void Add_ThrowsNotSupported()
    {
        var filePath = CreateTestTextSoxFile();

        Assert.Throws<NotSupportedException>(() =>
            _patcher.Add(filePath, new Dictionary<string, object>()));
    }

    [Fact]
    public void Delete_ThrowsNotSupported()
    {
        var filePath = CreateTestTextSoxFile();

        Assert.Throws<NotSupportedException>(() =>
            _patcher.Delete(filePath, "0"));
    }

    private string CreateTestTextSoxFile()
    {
        var filePath = Path.Combine(_testDir, "ItemTypeInfo_ENG.sox");

        using var fs = File.Create(filePath);

        // Create 3 entries with length prefix format.
        var entries = new[] { "Test Entry 1", "Second Entry", "Third Entry" };
        var encoding = Encoding.GetEncoding(1252);

        foreach (var entry in entries)
        {
            byte maxLen = 20;
            fs.WriteByte(maxLen);

            var textBytes = new byte[maxLen];
            var sourceBytes = encoding.GetBytes(entry);
            Array.Copy(sourceBytes, textBytes, Math.Min(sourceBytes.Length, maxLen));
            fs.Write(textBytes, 0, maxLen);
        }

        return filePath;
    }
}

public class SoxPatcherRegistryTests
{
    private readonly SoxPatcherRegistry _registry;

    public SoxPatcherRegistryTests()
    {
        _registry = new SoxPatcherRegistry();
    }

    [Fact]
    public void CanHandle_ReturnsTrueForSupportedFiles()
    {
        Assert.True(_registry.CanHandle("TroopInfo.sox"));
        Assert.True(_registry.CanHandle("ItemTypeInfo_ENG.sox"));
    }

    [Fact]
    public void CanHandle_ReturnsFalseForUnsupportedFiles()
    {
        Assert.False(_registry.CanHandle("SkillInfo.sox"));
        Assert.False(_registry.CanHandle("Unknown.sox"));
    }
}

public class TroopInfoDiffGeneratorTests : IDisposable
{
    private readonly string _testDir;
    private readonly TroopInfoDiffGenerator _generator;
    private readonly TroopInfoSoxFile _parser;

    public TroopInfoDiffGeneratorTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"DiffTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDir);
        _generator = new TroopInfoDiffGenerator();
        _parser = new TroopInfoSoxFile();
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
            Directory.Delete(_testDir, recursive: true);
    }

    [Fact]
    public void CanHandle_ReturnsTrueForTroopInfoSox()
    {
        Assert.True(_generator.CanHandle("TroopInfo.sox"));
    }

    [Fact]
    public void CanHandle_ReturnsFalseForOtherFiles()
    {
        Assert.False(_generator.CanHandle("SkillInfo.sox"));
    }

    [Fact]
    public void GenerateDiff_ReturnsEmptyForIdenticalFiles()
    {
        var originalPath = CreateTroopInfoFile("original.sox");
        var modifiedPath = CreateTroopInfoFile("modified.sox");

        var patches = _generator.GenerateDiff(originalPath, modifiedPath, "Data/SOX/TroopInfo.sox");

        Assert.Empty(patches);
    }

    [Fact]
    public void GenerateDiff_DetectsSingleFieldChange()
    {
        var originalPath = CreateTroopInfoFile("original.sox");
        var modifiedPath = CreateTroopInfoFile("modified.sox");

        // Modify the first troop's MoveSpeed.
        var sox = (TroopInfoSox)_parser.Read(modifiedPath);
        sox.TroopInfos[0].MoveSpeed = 999.0f;
        _parser.Write(modifiedPath, sox);

        var patches = _generator.GenerateDiff(originalPath, modifiedPath, "Data/SOX/TroopInfo.sox");

        Assert.Single(patches);
        Assert.Equal("Data/SOX/TroopInfo.sox", patches[0].File);
        Assert.Equal("Archer", patches[0].Record);
        Assert.Single(patches[0].Fields!);
        Assert.Equal(999.0f, patches[0].Fields!["MoveSpeed"]);
    }

    [Fact]
    public void GenerateDiff_DetectsMultipleFieldChanges()
    {
        var originalPath = CreateTroopInfoFile("original.sox");
        var modifiedPath = CreateTroopInfoFile("modified.sox");

        var sox = (TroopInfoSox)_parser.Read(modifiedPath);
        sox.TroopInfos[2].MoveSpeed = 10.0f;
        sox.TroopInfos[2].Defense = 20.0f;
        sox.TroopInfos[2].DirectAttack = 30.0f;
        _parser.Write(modifiedPath, sox);

        var patches = _generator.GenerateDiff(originalPath, modifiedPath, "Data/SOX/TroopInfo.sox");

        Assert.Single(patches);
        Assert.Equal("Infantry", patches[0].Record);
        Assert.Equal(3, patches[0].Fields!.Count);
    }

    [Fact]
    public void GenerateDiff_DetectsChangesInMultipleTroops()
    {
        var originalPath = CreateTroopInfoFile("original.sox");
        var modifiedPath = CreateTroopInfoFile("modified.sox");

        var sox = (TroopInfoSox)_parser.Read(modifiedPath);
        sox.TroopInfos[0].MoveSpeed = 100.0f;
        sox.TroopInfos[5].Defense = 200.0f;
        _parser.Write(modifiedPath, sox);

        var patches = _generator.GenerateDiff(originalPath, modifiedPath, "Data/SOX/TroopInfo.sox");

        Assert.Equal(2, patches.Count);
    }

    private string CreateTroopInfoFile(string fileName)
    {
        var filePath = Path.Combine(_testDir, fileName);
        var sox = CreateTestTroopInfoSox();
        _parser.Write(filePath, sox);

        var backupPath = filePath + ".bak";
        if (File.Exists(backupPath))
            File.Delete(backupPath);

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
                Defense = 10.0f,
                DirectAttack = 5.0f
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

public class DiffGeneratorRegistryTests
{
    private readonly DiffGeneratorRegistry _registry;

    public DiffGeneratorRegistryTests()
    {
        _registry = new DiffGeneratorRegistry();
    }

    [Fact]
    public void CanHandle_ReturnsTrueForSupportedFiles()
    {
        Assert.True(_registry.CanHandle("TroopInfo.sox"));
        Assert.True(_registry.CanHandle("ItemTypeInfo_ENG.sox"));
    }

    [Fact]
    public void CanHandle_ReturnsFalseForUnsupportedFiles()
    {
        Assert.False(_registry.CanHandle("SkillInfo.sox"));
    }
}
