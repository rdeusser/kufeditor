using Xunit;
using KUFEditor.Core;

namespace KUFEditor.Core.Tests;

public class SettingsTests : IDisposable
{
    private readonly string _testDir;

    public SettingsTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"SettingsTests_{Guid.NewGuid():N}");
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
    public void GetDefaultBackupDirectory_ReturnsValidPath()
    {
        var path = Settings.GetDefaultBackupDirectory();

        Assert.False(string.IsNullOrEmpty(path));
        Assert.Contains("KUFBackup", path);
    }

    [Fact]
    public void GetDefaultBackupDirectory_ReturnsAbsolutePath()
    {
        var path = Settings.GetDefaultBackupDirectory();

        Assert.True(Path.IsPathRooted(path));
    }

    [Fact]
    public void GetGameBackupDirectory_CombinesPathsCorrectly()
    {
        var backupRoot = "/tmp/backups";
        var gameName = "Crusaders";

        var result = Settings.GetGameBackupDirectory(backupRoot, gameName);

        Assert.Equal(Path.Combine("/tmp/backups", "Crusaders"), result);
    }

    [Fact]
    public void Save_CreatesFile()
    {
        var settings = new Settings
        {
            CrusadersPath = "/games/crusaders",
            HeroesPath = "/games/heroes",
            BackupDirectory = "/backups"
        };
        var filePath = Path.Combine(_testDir, "settings.json");

        settings.Save(filePath);

        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public void Save_CreatesDirectory()
    {
        var settings = new Settings();
        var filePath = Path.Combine(_testDir, "subdir", "settings.json");

        settings.Save(filePath);

        Assert.True(Directory.Exists(Path.GetDirectoryName(filePath)));
        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public void Load_ReturnsDefaultsWhenFileNotExists()
    {
        var filePath = Path.Combine(_testDir, "nonexistent.json");

        var settings = Settings.Load(filePath);

        Assert.NotNull(settings);
        Assert.Null(settings.CrusadersPath);
        Assert.Null(settings.HeroesPath);
        Assert.NotNull(settings.BackupDirectory);
    }

    [Fact]
    public void Load_SetsDefaultBackupDirectory()
    {
        var filePath = Path.Combine(_testDir, "nonexistent.json");

        var settings = Settings.Load(filePath);

        Assert.Equal(Settings.GetDefaultBackupDirectory(), settings.BackupDirectory);
    }

    [Fact]
    public void SaveAndLoad_RoundTripsData()
    {
        var original = new Settings
        {
            CrusadersPath = "/games/crusaders",
            HeroesPath = "/games/heroes",
            BackupDirectory = "/custom/backups"
        };
        var filePath = Path.Combine(_testDir, "settings.json");

        original.Save(filePath);
        var loaded = Settings.Load(filePath);

        Assert.Equal("/games/crusaders", loaded.CrusadersPath);
        Assert.Equal("/games/heroes", loaded.HeroesPath);
        Assert.Equal("/custom/backups", loaded.BackupDirectory);
    }

    [Fact]
    public void Load_SetsBackupDirectoryWhenMissing()
    {
        // Create a settings file without BackupDirectory
        var filePath = Path.Combine(_testDir, "settings.json");
        File.WriteAllText(filePath, """
        {
            "CrusadersPath": "/games/crusaders",
            "HeroesPath": "/games/heroes"
        }
        """);

        var settings = Settings.Load(filePath);

        Assert.NotNull(settings.BackupDirectory);
        Assert.Equal(Settings.GetDefaultBackupDirectory(), settings.BackupDirectory);
    }

    [Fact]
    public void Load_PreservesNullGamePaths()
    {
        var filePath = Path.Combine(_testDir, "settings.json");
        File.WriteAllText(filePath, """
        {
            "BackupDirectory": "/backups"
        }
        """);

        var settings = Settings.Load(filePath);

        Assert.Null(settings.CrusadersPath);
        Assert.Null(settings.HeroesPath);
    }

    [Fact]
    public void GetDefaultSettingsPath_ReturnsValidPath()
    {
        var path = Settings.GetDefaultSettingsPath();

        Assert.False(string.IsNullOrEmpty(path));
        Assert.Contains("KUFEditor", path);
        Assert.EndsWith("settings.json", path);
    }

    [Fact]
    public void GetDefaultSettingsPath_ReturnsAbsolutePath()
    {
        var path = Settings.GetDefaultSettingsPath();

        Assert.True(Path.IsPathRooted(path));
    }
}
