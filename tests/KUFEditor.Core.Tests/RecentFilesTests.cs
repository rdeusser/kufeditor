using Xunit;
using KUFEditor.Core.RecentFiles;

namespace KUFEditor.Core.Tests;

public class RecentFilesManagerTests
{
    [Fact]
    public void Constructor_SetsMaxFiles()
    {
        var manager = new RecentFilesManager(5);

        Assert.Equal(5, manager.MaxFiles);
    }

    [Fact]
    public void Constructor_ThrowsOnInvalidMaxFiles()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new RecentFilesManager(0));
        Assert.Throws<ArgumentOutOfRangeException>(() => new RecentFilesManager(-1));
    }

    [Fact]
    public void Add_AddsFile()
    {
        var manager = new RecentFilesManager();

        manager.Add("/path/to/file.sox");

        Assert.Single(manager.Files);
        Assert.Equal("/path/to/file.sox", manager.Files[0].Path);
    }

    [Fact]
    public void Add_MovesExistingFileToTop()
    {
        var manager = new RecentFilesManager();

        manager.Add("/file1.sox");
        manager.Add("/file2.sox");
        manager.Add("/file1.sox");

        Assert.Equal(2, manager.Count);
        Assert.Equal("/file1.sox", manager.Files[0].Path);
        Assert.Equal("/file2.sox", manager.Files[1].Path);
    }

    [Fact]
    public void Add_TrimsToMaxSize()
    {
        var manager = new RecentFilesManager(3);

        manager.Add("/file1.sox");
        manager.Add("/file2.sox");
        manager.Add("/file3.sox");
        manager.Add("/file4.sox");

        Assert.Equal(3, manager.Count);
        Assert.Equal("/file4.sox", manager.Files[0].Path);
        Assert.Equal("/file3.sox", manager.Files[1].Path);
        Assert.Equal("/file2.sox", manager.Files[2].Path);
    }

    [Fact]
    public void Add_SetsLastOpenedTime()
    {
        var manager = new RecentFilesManager();
        var before = DateTime.UtcNow;

        manager.Add("/file.sox");

        var after = DateTime.UtcNow;
        Assert.InRange(manager.Files[0].LastOpened, before, after);
    }

    [Fact]
    public void Add_RaisesChangedEvent()
    {
        var manager = new RecentFilesManager();
        var eventRaised = false;
        manager.Changed += (s, e) => eventRaised = true;

        manager.Add("/file.sox");

        Assert.True(eventRaised);
    }

    [Fact]
    public void Remove_RemovesFile()
    {
        var manager = new RecentFilesManager();
        manager.Add("/file.sox");

        var result = manager.Remove("/file.sox");

        Assert.True(result);
        Assert.Empty(manager.Files);
    }

    [Fact]
    public void Remove_ReturnsFalseForNonexistent()
    {
        var manager = new RecentFilesManager();

        var result = manager.Remove("/nonexistent.sox");

        Assert.False(result);
    }

    [Fact]
    public void Remove_IsCaseInsensitive()
    {
        var manager = new RecentFilesManager();
        manager.Add("/File.sox");

        var result = manager.Remove("/FILE.SOX");

        Assert.True(result);
    }

    [Fact]
    public void Clear_RemovesAllFiles()
    {
        var manager = new RecentFilesManager();
        manager.Add("/file1.sox");
        manager.Add("/file2.sox");

        manager.Clear();

        Assert.Empty(manager.Files);
    }

    [Fact]
    public void Clear_RaisesChangedEvent()
    {
        var manager = new RecentFilesManager();
        manager.Add("/file.sox");
        var eventRaised = false;
        manager.Changed += (s, e) => eventRaised = true;

        manager.Clear();

        Assert.True(eventRaised);
    }

    [Fact]
    public void Clear_DoesNotRaiseEventIfEmpty()
    {
        var manager = new RecentFilesManager();
        var eventRaised = false;
        manager.Changed += (s, e) => eventRaised = true;

        manager.Clear();

        Assert.False(eventRaised);
    }

    [Fact]
    public void Contains_ReturnsTrueForExistingFile()
    {
        var manager = new RecentFilesManager();
        manager.Add("/file.sox");

        Assert.True(manager.Contains("/file.sox"));
    }

    [Fact]
    public void Contains_IsCaseInsensitive()
    {
        var manager = new RecentFilesManager();
        manager.Add("/File.sox");

        Assert.True(manager.Contains("/FILE.SOX"));
    }

    [Fact]
    public void Contains_ReturnsFalseForNonexistent()
    {
        var manager = new RecentFilesManager();

        Assert.False(manager.Contains("/nonexistent.sox"));
    }

    [Fact]
    public void GetMostRecent_ReturnsFirstFile()
    {
        var manager = new RecentFilesManager();
        manager.Add("/file1.sox");
        manager.Add("/file2.sox");

        var result = manager.GetMostRecent();

        Assert.NotNull(result);
        Assert.Equal("/file2.sox", result.Path);
    }

    [Fact]
    public void GetMostRecent_ReturnsNullWhenEmpty()
    {
        var manager = new RecentFilesManager();

        var result = manager.GetMostRecent();

        Assert.Null(result);
    }

    [Fact]
    public void SaveAndLoad_RoundTripsData()
    {
        var path = Path.Combine(Path.GetTempPath(), $"recent_test_{Guid.NewGuid()}.json");
        try
        {
            var manager1 = new RecentFilesManager();
            manager1.Add("/file1.sox");
            manager1.Add("/file2.sox");
            manager1.Save(path);

            var manager2 = new RecentFilesManager();
            manager2.Load(path);

            Assert.Equal(2, manager2.Count);
            Assert.Equal("/file2.sox", manager2.Files[0].Path);
            Assert.Equal("/file1.sox", manager2.Files[1].Path);
        }
        finally
        {
            if (File.Exists(path))
                File.Delete(path);
        }
    }

    [Fact]
    public void Load_HandlesNonexistentFile()
    {
        var manager = new RecentFilesManager();

        manager.Load("/nonexistent/path.json");

        Assert.Empty(manager.Files);
    }

    [Fact]
    public void Load_HandlesCorruptFile()
    {
        var path = Path.Combine(Path.GetTempPath(), $"corrupt_{Guid.NewGuid()}.json");
        try
        {
            File.WriteAllText(path, "not valid json {{{");

            var manager = new RecentFilesManager();
            manager.Load(path);

            Assert.Empty(manager.Files);
        }
        finally
        {
            if (File.Exists(path))
                File.Delete(path);
        }
    }

    [Fact]
    public void Load_RespectsMaxFiles()
    {
        var path = Path.Combine(Path.GetTempPath(), $"recent_test_{Guid.NewGuid()}.json");
        try
        {
            var manager1 = new RecentFilesManager(10);
            for (int i = 0; i < 10; i++)
            {
                manager1.Add($"/file{i}.sox");
            }
            manager1.Save(path);

            var manager2 = new RecentFilesManager(3);
            manager2.Load(path);

            Assert.Equal(3, manager2.Count);
        }
        finally
        {
            if (File.Exists(path))
                File.Delete(path);
        }
    }

    [Fact]
    public void Save_CreatesDirectory()
    {
        var dir = Path.Combine(Path.GetTempPath(), $"recent_test_{Guid.NewGuid()}");
        var path = Path.Combine(dir, "recent.json");
        try
        {
            var manager = new RecentFilesManager();
            manager.Add("/file.sox");
            manager.Save(path);

            Assert.True(File.Exists(path));
        }
        finally
        {
            if (Directory.Exists(dir))
                Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void RemoveNonExistent_RemovesDeletedFiles()
    {
        var path = Path.Combine(Path.GetTempPath(), $"test_file_{Guid.NewGuid()}.sox");
        try
        {
            File.WriteAllText(path, "test");

            var manager = new RecentFilesManager();
            manager.Add(path);
            manager.Add("/nonexistent/file.sox");

            Assert.Equal(2, manager.Count);

            var removed = manager.RemoveNonExistent();

            Assert.Equal(1, removed);
            Assert.Single(manager.Files);
            Assert.Equal(path, manager.Files[0].Path);
        }
        finally
        {
            if (File.Exists(path))
                File.Delete(path);
        }
    }

    [Fact]
    public void GetDefaultPath_ReturnsValidPath()
    {
        var path = RecentFilesManager.GetDefaultPath();

        Assert.NotEmpty(path);
        Assert.EndsWith("recent_files.json", path);
    }
}

public class RecentFileTests
{
    [Fact]
    public void Constructor_SetsProperties()
    {
        var time = DateTime.UtcNow;
        var testPath = Path.Combine("path", "to", "file.sox");
        var file = new RecentFile(testPath, time);

        Assert.Equal(testPath, file.Path);
        Assert.Equal(time, file.LastOpened);
    }

    [Fact]
    public void FileName_ReturnsFileNameOnly()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var file = new RecentFile(testPath, DateTime.UtcNow);

        Assert.Equal("file.sox", file.FileName);
    }

    [Fact]
    public void Directory_ReturnsDirectoryPath()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var expectedDir = Path.Combine("path", "to");
        var file = new RecentFile(testPath, DateTime.UtcNow);

        Assert.Equal(expectedDir, file.Directory);
    }

    [Fact]
    public void DefaultConstructor_CreatesEmptyFile()
    {
        var file = new RecentFile();

        Assert.Equal(string.Empty, file.Path);
        Assert.Equal(default(DateTime), file.LastOpened);
    }
}
