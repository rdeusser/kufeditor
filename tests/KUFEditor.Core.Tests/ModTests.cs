using System.Text.Json;
using Xunit;
using KUFEditor.Core.Mods;

namespace KUFEditor.Core.Tests;

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
