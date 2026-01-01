using Xunit;
using KUFEditor.Core.Files;

namespace KUFEditor.Core.Tests;

public class OpenFileTests
{
    [Fact]
    public void Constructor_SetsPath()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var file = new OpenFile(testPath);

        Assert.Equal(testPath, file.Path);
    }

    [Fact]
    public void Name_ReturnsFileNameOnly()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var file = new OpenFile(testPath);

        Assert.Equal("file.sox", file.Name);
    }

    [Fact]
    public void Extension_ReturnsExtension()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var file = new OpenFile(testPath);

        Assert.Equal(".sox", file.Extension);
    }

    [Fact]
    public void Directory_ReturnsDirectoryPath()
    {
        var testPath = Path.Combine("path", "to", "file.sox");
        var expectedDir = Path.Combine("path", "to");
        var file = new OpenFile(testPath);

        Assert.Equal(expectedDir, file.Directory);
    }

    [Fact]
    public void IsDirty_InitiallyFalse()
    {
        var file = new OpenFile("/file.sox");

        Assert.False(file.IsDirty);
    }

    [Fact]
    public void IsDirty_RaisesDirtyChangedEvent()
    {
        var file = new OpenFile("/file.sox");
        var eventRaised = false;
        file.DirtyChanged += (s, e) => eventRaised = true;

        file.IsDirty = true;

        Assert.True(eventRaised);
    }

    [Fact]
    public void IsDirty_DoesNotRaiseEventWhenUnchanged()
    {
        var file = new OpenFile("/file.sox");
        file.IsDirty = true;
        var eventRaised = false;
        file.DirtyChanged += (s, e) => eventRaised = true;

        file.IsDirty = true;

        Assert.False(eventRaised);
    }

    [Fact]
    public void Data_SettingMarksDirty()
    {
        var file = new OpenFile("/file.sox");

        file.Data = new object();

        Assert.True(file.IsDirty);
    }

    [Fact]
    public void Title_IncludesAsteriskWhenDirty()
    {
        var file = new OpenFile("/file.sox");
        file.IsDirty = true;

        Assert.Equal("file.sox*", file.Title);
    }

    [Fact]
    public void Title_NoAsteriskWhenClean()
    {
        var file = new OpenFile("/file.sox");

        Assert.Equal("file.sox", file.Title);
    }

    [Fact]
    public void Save_CallsSaveHandler()
    {
        var file = new OpenFile("/file.sox");
        var handlerCalled = false;
        file.SetSaveHandler(f => handlerCalled = true);

        file.Save();

        Assert.True(handlerCalled);
    }

    [Fact]
    public void Save_ClearsDirtyFlag()
    {
        var file = new OpenFile("/file.sox");
        file.IsDirty = true;

        file.Save();

        Assert.False(file.IsDirty);
    }

    [Fact]
    public void Save_SetsLastSavedAt()
    {
        var file = new OpenFile("/file.sox");
        var before = DateTime.UtcNow;

        file.Save();

        Assert.NotNull(file.LastSavedAt);
        Assert.True(file.LastSavedAt >= before);
    }

    [Fact]
    public void Save_RaisesSavedEvent()
    {
        var file = new OpenFile("/file.sox");
        var eventRaised = false;
        file.Saved += (s, e) => eventRaised = true;

        file.Save();

        Assert.True(eventRaised);
    }

    [Fact]
    public void MarkDirty_SetsDirtyTrue()
    {
        var file = new OpenFile("/file.sox");

        file.MarkDirty();

        Assert.True(file.IsDirty);
    }

    [Fact]
    public void MarkClean_SetsDirtyFalse()
    {
        var file = new OpenFile("/file.sox");
        file.IsDirty = true;

        file.MarkClean();

        Assert.False(file.IsDirty);
    }

    [Fact]
    public void OpenedAt_IsSetOnConstruction()
    {
        var before = DateTime.UtcNow;
        var file = new OpenFile("/file.sox");
        var after = DateTime.UtcNow;

        Assert.InRange(file.OpenedAt, before, after);
    }

    [Theory]
    [InlineData("/game/TroopInfo.sox", FileType.TroopInfoSox)]
    [InlineData("/game/SkillInfo.sox", FileType.SkillInfoSox)]
    [InlineData("/game/ExpInfo.sox", FileType.ExpInfoSox)]
    [InlineData("/game/ItemTypeInfo_ENG.sox", FileType.TextSox)]
    [InlineData("/game/CharName_KOR.sox", FileType.TextSox)]
    [InlineData("/game/Other.sox", FileType.BinarySox)]
    [InlineData("/game/Mission.stg", FileType.Mission)]
    [InlineData("/game/Save.sav", FileType.SaveGame)]
    [InlineData("/game/Map.nav", FileType.Navigation)]
    [InlineData("/game/Model.k2a", FileType.Model)]
    [InlineData("/game/readme.txt", FileType.Text)]
    [InlineData("/game/config.xml", FileType.Xml)]
    [InlineData("/game/unknown.dat", FileType.Unknown)]
    public void Type_DeterminedFromPath(string path, FileType expected)
    {
        var file = new OpenFile(path);

        Assert.Equal(expected, file.Type);
    }
}

public class FileManagerTests
{
    [Fact]
    public void Open_CreatesNewFile()
    {
        var manager = new FileManager();

        var file = manager.Open("/file.sox");

        Assert.NotNull(file);
        Assert.Equal("/file.sox", file.Path);
    }

    [Fact]
    public void Open_ReturnsExistingFile()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file.sox");

        var file2 = manager.Open("/file.sox");

        Assert.Same(file1, file2);
    }

    [Fact]
    public void Open_SetsActiveFile()
    {
        var manager = new FileManager();

        var file = manager.Open("/file.sox");

        Assert.Same(file, manager.ActiveFile);
    }

    [Fact]
    public void Open_RaisesFileOpenedEvent()
    {
        var manager = new FileManager();
        OpenFile? opened = null;
        manager.FileOpened += (s, f) => opened = f;

        var file = manager.Open("/file.sox");

        Assert.Same(file, opened);
    }

    [Fact]
    public void Close_RemovesFile()
    {
        var manager = new FileManager();
        manager.Open("/file.sox");

        var result = manager.Close("/file.sox");

        Assert.True(result);
        Assert.Empty(manager.Files);
    }

    [Fact]
    public void Close_ReturnsFalseForUnknown()
    {
        var manager = new FileManager();

        var result = manager.Close("/unknown.sox");

        Assert.False(result);
    }

    [Fact]
    public void Close_RaisesFileClosedEvent()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        OpenFile? closed = null;
        manager.FileClosed += (s, f) => closed = f;

        manager.Close("/file.sox");

        Assert.Same(file, closed);
    }

    [Fact]
    public void Close_UpdatesActiveFile()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");

        manager.Close("/file2.sox");

        Assert.Same(file1, manager.ActiveFile);
    }

    [Fact]
    public void Close_WithDirtyFile_ReturnsFalseUnlessForced()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        file.IsDirty = true;

        Assert.False(manager.Close(file, force: false));
        Assert.True(manager.Close(file, force: true));
    }

    [Fact]
    public void Get_ReturnsFile()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");

        var result = manager.Get("/file.sox");

        Assert.Same(file, result);
    }

    [Fact]
    public void Get_ReturnsNullForUnknown()
    {
        var manager = new FileManager();

        var result = manager.Get("/unknown.sox");

        Assert.Null(result);
    }

    [Fact]
    public void IsOpen_ReturnsTrueForOpenFile()
    {
        var manager = new FileManager();
        manager.Open("/file.sox");

        Assert.True(manager.IsOpen("/file.sox"));
    }

    [Fact]
    public void IsOpen_ReturnsFalseForClosedFile()
    {
        var manager = new FileManager();

        Assert.False(manager.IsOpen("/file.sox"));
    }

    [Fact]
    public void SetActive_ChangesActiveFile()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");

        manager.SetActive(file1);

        Assert.Same(file1, manager.ActiveFile);
    }

    [Fact]
    public void SetActive_RaisesActiveFileChangedEvent()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");
        OpenFile? changed = null;
        manager.ActiveFileChanged += (s, f) => changed = f;

        manager.SetActive(file1);

        Assert.Same(file1, changed);
    }

    [Fact]
    public void SetActive_ByPath_Works()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        manager.Open("/file2.sox");

        var result = manager.SetActive("/file1.sox");

        Assert.True(result);
        Assert.Same(file1, manager.ActiveFile);
    }

    [Fact]
    public void Save_SavesActiveFile()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        var saved = false;
        file.SetSaveHandler(f => saved = true);
        file.IsDirty = true;

        manager.Save();

        Assert.True(saved);
    }

    [Fact]
    public void Save_ReturnsFalseWhenNoActiveFile()
    {
        var manager = new FileManager();

        var result = manager.Save();

        Assert.False(result);
    }

    [Fact]
    public void Save_RaisesFileSavedEvent()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        file.IsDirty = true;
        OpenFile? saved = null;
        manager.FileSaved += (s, f) => saved = f;

        manager.Save(file);

        Assert.Same(file, saved);
    }

    [Fact]
    public void SaveAll_SavesAllDirtyFiles()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");
        var file3 = manager.Open("/file3.sox");

        file1.IsDirty = true;
        file2.IsDirty = true;

        var count = manager.SaveAll();

        Assert.Equal(2, count);
        Assert.False(file1.IsDirty);
        Assert.False(file2.IsDirty);
    }

    [Fact]
    public void HasUnsavedChanges_TrueWhenAnyDirty()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        file.IsDirty = true;

        Assert.True(manager.HasUnsavedChanges);
    }

    [Fact]
    public void HasUnsavedChanges_FalseWhenAllClean()
    {
        var manager = new FileManager();
        manager.Open("/file.sox");

        Assert.False(manager.HasUnsavedChanges);
    }

    [Fact]
    public void DirtyFiles_ReturnsOnlyDirty()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");
        file1.IsDirty = true;

        var dirty = manager.DirtyFiles.ToList();

        Assert.Single(dirty);
        Assert.Same(file1, dirty[0]);
    }

    [Fact]
    public void CloseAll_ClosesAllFiles()
    {
        var manager = new FileManager();
        manager.Open("/file1.sox");
        manager.Open("/file2.sox");

        manager.CloseAll(force: true);

        Assert.Empty(manager.Files);
    }

    [Fact]
    public void CloseAll_FailsWithUnsavedChanges()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        file.IsDirty = true;

        var result = manager.CloseAll(force: false);

        Assert.False(result);
        Assert.Single(manager.Files);
    }

    [Fact]
    public void NextFile_CyclesThrough()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");
        var file3 = manager.Open("/file3.sox");

        manager.SetActive(file1);
        var next = manager.NextFile();

        Assert.Same(file2, next);
    }

    [Fact]
    public void NextFile_WrapsAround()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");

        manager.SetActive(file2);
        var next = manager.NextFile();

        Assert.Same(file1, next);
    }

    [Fact]
    public void PreviousFile_CyclesBackward()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");
        var file3 = manager.Open("/file3.sox");

        manager.SetActive(file3);
        var prev = manager.PreviousFile();

        Assert.Same(file2, prev);
    }

    [Fact]
    public void PreviousFile_WrapsAround()
    {
        var manager = new FileManager();
        var file1 = manager.Open("/file1.sox");
        var file2 = manager.Open("/file2.sox");

        manager.SetActive(file1);
        var prev = manager.PreviousFile();

        Assert.Same(file2, prev);
    }

    [Fact]
    public void FileDirtyChanged_FiresWhenFileChanges()
    {
        var manager = new FileManager();
        var file = manager.Open("/file.sox");
        OpenFile? changed = null;
        manager.FileDirtyChanged += (s, f) => changed = f;

        file.IsDirty = true;

        Assert.Same(file, changed);
    }

    [Fact]
    public void Count_ReturnsNumberOfFiles()
    {
        var manager = new FileManager();
        manager.Open("/file1.sox");
        manager.Open("/file2.sox");

        Assert.Equal(2, manager.Count);
    }
}
