using System.Text.Json;
using Xunit;
using KUFEditor.Core.Mods;

namespace KUFEditor.Core.Tests;

public class ModManagerTests
{
    private string GetTempDir() => Path.Combine(Path.GetTempPath(), $"modtest_{Guid.NewGuid()}");

    [Fact]
    public void GetEnabledMods_ReturnsEmptyForNewGame()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            var enabled = manager.GetEnabledMods("Crusaders");
            Assert.Empty(enabled);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void EnableMod_AddToList()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            manager.EnableMod("Crusaders", "test-mod");

            Assert.True(manager.IsEnabled("Crusaders", "test-mod"));
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void SaveAndLoad_PersistsState()
    {
        var dir = GetTempDir();
        try
        {
            var manager1 = new ModManager(dir);
            manager1.EnableMod("Crusaders", "mod-a");
            manager1.EnableMod("Crusaders", "mod-b");

            var manager2 = new ModManager(dir);
            var enabled = manager2.GetEnabledMods("Crusaders");

            Assert.Equal(2, enabled.Count);
            Assert.Equal("mod-a", enabled[0]);
            Assert.Equal("mod-b", enabled[1]);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void ReorderMod_ChangesPosition()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            manager.EnableMod("Crusaders", "mod-a");
            manager.EnableMod("Crusaders", "mod-b");
            manager.EnableMod("Crusaders", "mod-c");

            manager.ReorderMod("Crusaders", "mod-c", 0);

            var enabled = manager.GetEnabledMods("Crusaders");
            Assert.Equal("mod-c", enabled[0]);
            Assert.Equal("mod-a", enabled[1]);
            Assert.Equal("mod-b", enabled[2]);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void DisableMod_RemovesFromList()
    {
        var dir = GetTempDir();
        try
        {
            var manager = new ModManager(dir);
            manager.EnableMod("Crusaders", "mod-a");
            manager.EnableMod("Crusaders", "mod-b");

            manager.DisableMod("Crusaders", "mod-a");

            var enabled = manager.GetEnabledMods("Crusaders");
            Assert.Single(enabled);
            Assert.Equal("mod-b", enabled[0]);
        }
        finally
        {
            if (Directory.Exists(dir)) Directory.Delete(dir, true);
        }
    }
}

public class ModTests
{
    [Fact]
    public void Deserialize_ParsesModJson()
    {
        var json = """
        {
            "id": "test-mod",
            "name": "Test Mod",
            "version": "1.0.0",
            "author": "Tester",
            "description": "A test mod",
            "game": "Crusaders",
            "patches": [
                {
                    "file": "TroopInfo.sox",
                    "action": "Modify",
                    "record": "Gerald",
                    "fields": { "HP": 500 }
                }
            ]
        }
        """;

        var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
        var mod = JsonSerializer.Deserialize<Mod>(json, options);

        Assert.NotNull(mod);
        Assert.Equal("test-mod", mod.Id);
        Assert.Equal("Test Mod", mod.Name);
        Assert.Single(mod.Patches);
        Assert.Equal("TroopInfo.sox", mod.Patches[0].File);
        Assert.Equal(PatchAction.Modify, mod.Patches[0].Action);
    }

    [Fact]
    public void Deserialize_ParsesAllPatchActions()
    {
        var json = """
        {
            "id": "action-test",
            "name": "Action Test",
            "version": "1.0.0",
            "author": "Tester",
            "description": "Tests all actions",
            "game": "Crusaders",
            "patches": [
                { "file": "a.sox", "action": "Modify", "record": "A", "fields": {} },
                { "file": "b.sox", "action": "Add", "data": {} },
                { "file": "c.sox", "action": "Delete", "record": "C" }
            ]
        }
        """;

        var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true };
        var mod = JsonSerializer.Deserialize<Mod>(json, options);

        Assert.NotNull(mod);
        Assert.Equal(3, mod.Patches.Count);
        Assert.Equal(PatchAction.Modify, mod.Patches[0].Action);
        Assert.Equal(PatchAction.Add, mod.Patches[1].Action);
        Assert.Equal(PatchAction.Delete, mod.Patches[2].Action);
    }
}
