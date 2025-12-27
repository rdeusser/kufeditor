using Xunit;
using KUFEditor.Core;

namespace KUFEditor.Core.Tests;

public class BackupManagerTests : IDisposable
{
    private readonly string _testDir;
    private readonly string _backupRoot;
    private readonly string _gameDir;
    private readonly BackupManager _manager;

    public BackupManagerTests()
    {
        _testDir = Path.Combine(Path.GetTempPath(), $"KUFEditorTests_{Guid.NewGuid():N}");
        _backupRoot = Path.Combine(_testDir, "Backups");
        _gameDir = Path.Combine(_testDir, "Game");

        Directory.CreateDirectory(_backupRoot);
        Directory.CreateDirectory(_gameDir);

        _manager = new BackupManager(_backupRoot);
    }

    public void Dispose()
    {
        if (Directory.Exists(_testDir))
        {
            Directory.Delete(_testDir, recursive: true);
        }
    }

    private string CreateTestFile(string relativePath, string content = "test content")
    {
        var fullPath = Path.Combine(_gameDir, relativePath);
        var dir = Path.GetDirectoryName(fullPath);
        if (dir != null && !Directory.Exists(dir))
            Directory.CreateDirectory(dir);

        File.WriteAllText(fullPath, content);
        return fullPath;
    }

    // ========== Pristine Backup Tests ==========

    [Fact]
    public void CapturePristine_CapturesSoxFiles()
    {
        // arrange
        CreateTestFile("Data/SOX/TroopInfo.sox", "troop data");
        CreateTestFile("Data/SOX/SkillInfo.sox", "skill data");

        // act
        _manager.CapturePristine(_gameDir, "Crusaders");

        // assert
        Assert.True(_manager.HasPristineBackup("Crusaders", "TroopInfo.sox"));
        Assert.True(_manager.HasPristineBackup("Crusaders", "SkillInfo.sox"));
    }

    [Fact]
    public void CapturePristine_CapturesMissionFiles()
    {
        // arrange
        CreateTestFile("Data/Mission/H0000.stg", "mission data");
        CreateTestFile("Data/Mission/Briefing/H0000.stg", "briefing data");

        // act
        _manager.CapturePristine(_gameDir, "Heroes");

        // assert
        Assert.True(_manager.HasPristineBackup("Heroes", "H0000.stg"));
    }

    [Fact]
    public void CapturePristine_DoesNotOverwriteExisting()
    {
        // arrange
        CreateTestFile("Data/SOX/TroopInfo.sox", "original content");
        _manager.CapturePristine(_gameDir, "Crusaders");

        // modify the source file
        CreateTestFile("Data/SOX/TroopInfo.sox", "modified content");

        // act - capture again
        _manager.CapturePristine(_gameDir, "Crusaders");

        // assert - pristine should still have original content
        var pristinePath = _manager.GetPristinePath("Crusaders", "TroopInfo.sox");
        Assert.NotNull(pristinePath);
        Assert.Equal("original content", File.ReadAllText(pristinePath));
    }

    [Fact]
    public void HasPristineBackup_ReturnsFalseWhenNotCaptured()
    {
        Assert.False(_manager.HasPristineBackup("Crusaders", "NonExistent.sox"));
    }

    [Fact]
    public void GetPristinePath_ReturnsNullWhenNotCaptured()
    {
        Assert.Null(_manager.GetPristinePath("Crusaders", "NonExistent.sox"));
    }

    [Fact]
    public void RestoreFromPristine_RestoresFile()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "original content");
        _manager.CapturePristine(_gameDir, "Crusaders");

        // modify the file
        File.WriteAllText(sourceFile, "modified content");

        // act
        _manager.RestoreFromPristine("Crusaders", "TroopInfo.sox", sourceFile);

        // assert
        Assert.Equal("original content", File.ReadAllText(sourceFile));
    }

    [Fact]
    public void RestoreFromPristine_ThrowsWhenNoPristine()
    {
        var destPath = Path.Combine(_gameDir, "test.sox");

        Assert.Throws<FileNotFoundException>(() =>
            _manager.RestoreFromPristine("Crusaders", "NonExistent.sox", destPath));
    }

    // ========== Named Snapshot Tests ==========

    [Fact]
    public void CreateSnapshot_CreatesSnapshot()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "snapshot content");

        // act
        var snapshotPath = _manager.CreateSnapshot("Crusaders", sourceFile, "Before changes");

        // assert
        Assert.True(File.Exists(snapshotPath));
        Assert.Equal("snapshot content", File.ReadAllText(snapshotPath));
    }

    [Fact]
    public void CreateSnapshot_ThrowsWhenSourceNotFound()
    {
        var nonExistentFile = Path.Combine(_gameDir, "nonexistent.sox");

        Assert.Throws<FileNotFoundException>(() =>
            _manager.CreateSnapshot("Crusaders", nonExistentFile, "test"));
    }

    [Fact]
    public void GetSnapshots_ReturnsEmptyWhenNone()
    {
        var snapshots = _manager.GetSnapshots("Crusaders", "TroopInfo.sox");

        Assert.Empty(snapshots);
    }

    [Fact]
    public void GetSnapshots_ReturnsAllSnapshots()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "content");
        _manager.CreateSnapshot("Crusaders", sourceFile, "Snapshot 1");
        _manager.CreateSnapshot("Crusaders", sourceFile, "Snapshot 2");
        _manager.CreateSnapshot("Crusaders", sourceFile, "Snapshot 3");

        // act
        var snapshots = _manager.GetSnapshots("Crusaders", "TroopInfo.sox");

        // assert
        Assert.Equal(3, snapshots.Count);
        Assert.Contains(snapshots, s => s.Name == "Snapshot 1");
        Assert.Contains(snapshots, s => s.Name == "Snapshot 2");
        Assert.Contains(snapshots, s => s.Name == "Snapshot 3");
    }

    [Fact]
    public void GetSnapshots_ReturnsMultipleSnapshotsWithCreationTime()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "content");
        _manager.CreateSnapshot("Crusaders", sourceFile, "First");
        _manager.CreateSnapshot("Crusaders", sourceFile, "Second");
        _manager.CreateSnapshot("Crusaders", sourceFile, "Third");

        // act
        var snapshots = _manager.GetSnapshots("Crusaders", "TroopInfo.sox");

        // assert - all snapshots returned with creation times
        Assert.Equal(3, snapshots.Count);
        Assert.All(snapshots, s => Assert.NotEqual(default, s.Created));
        var names = snapshots.Select(s => s.Name).ToList();
        Assert.Contains("First", names);
        Assert.Contains("Second", names);
        Assert.Contains("Third", names);
    }

    [Fact]
    public void RestoreSnapshot_RestoresFile()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "original content");
        var snapshotPath = _manager.CreateSnapshot("Crusaders", sourceFile, "Backup");

        // modify original
        File.WriteAllText(sourceFile, "modified content");

        // act
        _manager.RestoreSnapshot(snapshotPath, sourceFile);

        // assert
        Assert.Equal("original content", File.ReadAllText(sourceFile));
    }

    [Fact]
    public void RestoreSnapshot_ThrowsWhenSnapshotNotFound()
    {
        var destPath = Path.Combine(_gameDir, "test.sox");

        Assert.Throws<FileNotFoundException>(() =>
            _manager.RestoreSnapshot("/nonexistent/path", destPath));
    }

    [Fact]
    public void DeleteSnapshot_RemovesSnapshot()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "content");
        _manager.CreateSnapshot("Crusaders", sourceFile, "ToDelete");

        Assert.Single(_manager.GetSnapshots("Crusaders", "TroopInfo.sox"));

        // act
        _manager.DeleteSnapshot("Crusaders", "TroopInfo.sox", "ToDelete");

        // assert
        Assert.Empty(_manager.GetSnapshots("Crusaders", "TroopInfo.sox"));
    }

    [Fact]
    public void DeleteSnapshot_DoesNothingWhenNotFound()
    {
        // should not throw
        _manager.DeleteSnapshot("Crusaders", "TroopInfo.sox", "NonExistent");
    }

    // ========== Legacy Method Tests ==========

    [Fact]
    public void BackupFile_CreatesTimestampedBackup()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "backup content");

        // act
        var backupPath = _manager.BackupFile(sourceFile, "Crusaders");

        // assert
        Assert.True(File.Exists(backupPath));
        Assert.Equal("backup content", File.ReadAllText(backupPath));
        Assert.Contains("TroopInfo.sox", backupPath);
    }

    [Fact]
    public void GetBackups_ReturnsBackupDirectories()
    {
        // arrange
        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "content");
        _manager.BackupFile(sourceFile, "Crusaders");

        // act
        var backups = _manager.GetBackups("Crusaders");

        // assert
        Assert.Single(backups);
    }

    [Fact]
    public void CleanOldBackups_KeepsOnlySpecifiedCount()
    {
        // arrange - create backup directories manually to avoid timing issues
        var gameBackupDir = Path.Combine(_backupRoot, "Crusaders");
        Directory.CreateDirectory(gameBackupDir);

        var sourceFile = CreateTestFile("Data/SOX/TroopInfo.sox", "content");

        // create 5 backup directories with different timestamps in their names
        for (int i = 0; i < 5; i++)
        {
            var backupDir = Path.Combine(gameBackupDir, $"2024-01-0{i + 1}_12-00-00");
            Directory.CreateDirectory(backupDir);
            File.Copy(sourceFile, Path.Combine(backupDir, "TroopInfo.sox"));
        }

        Assert.Equal(5, _manager.GetBackups("Crusaders").Count);

        // act
        _manager.CleanOldBackups("Crusaders", keepCount: 2);

        // assert
        Assert.Equal(2, _manager.GetBackups("Crusaders").Count);
    }
}
