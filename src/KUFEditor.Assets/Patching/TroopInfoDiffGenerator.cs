using System;
using System.Collections.Generic;
using System.Reflection;
using KUFEditor.Assets.TroopInfo;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Generates diff patches for TroopInfo.sox files.
/// </summary>
public class TroopInfoDiffGenerator : IDiffGenerator
{
    private readonly TroopInfoSoxFile _parser = new();

    public bool CanHandle(string fileName)
    {
        return fileName.Equals("TroopInfo.sox", StringComparison.OrdinalIgnoreCase);
    }

    public List<ModPatch> GenerateDiff(string originalPath, string modifiedPath, string relativePath)
    {
        var patches = new List<ModPatch>();

        var original = (TroopInfoSox)_parser.Read(originalPath);
        var modified = (TroopInfoSox)_parser.Read(modifiedPath);

        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            var troopName = TroopNames.GetName(i);
            var diff = CompareTroops(original.TroopInfos[i], modified.TroopInfos[i]);

            if (diff.Count > 0)
            {
                patches.Add(new ModPatch
                {
                    File = relativePath,
                    Action = PatchAction.Modify,
                    Record = troopName,
                    Fields = diff
                });
            }
        }

        return patches;
    }

    private static Dictionary<string, object> CompareTroops(TroopInfo.TroopInfo original, TroopInfo.TroopInfo modified)
    {
        var diff = new Dictionary<string, object>();
        var type = typeof(TroopInfo.TroopInfo);

        // Compare simple properties.
        var simpleProps = new[]
        {
            "Job", "TypeID", "MoveSpeed", "RotateRate", "MoveAcceleration", "MoveDeceleration",
            "SightRange", "AttackRangeMax", "AttackRangeMin", "AttackFrontRange",
            "DirectAttack", "IndirectAttack", "Defense", "BaseWidth",
            "ResistMelee", "ResistRanged", "ResistFrontal", "ResistExplosion",
            "ResistFire", "ResistIce", "ResistLightning", "ResistHoly", "ResistCurse", "ResistPoison",
            "MaxUnitSpeedMultiplier", "DefaultUnitHP", "FormationRandom",
            "DefaultUnitNumX", "DefaultUnitNumY", "UnitHPLevUp", "DamageDistribution"
        };

        foreach (var propName in simpleProps)
        {
            var prop = type.GetProperty(propName);
            if (prop == null) continue;

            var origValue = prop.GetValue(original);
            var modValue = prop.GetValue(modified);

            if (!Equals(origValue, modValue))
            {
                diff[propName] = modValue!;
            }
        }

        // Compare LevelUpData.
        for (int i = 0; i < 3; i++)
        {
            var origData = original.LevelUpData[i];
            var modData = modified.LevelUpData[i];

            if (origData.SkillID != modData.SkillID)
            {
                diff[$"LevelUpData[{i}].SkillID"] = modData.SkillID;
            }
            if (!origData.SkillPerLevel.Equals(modData.SkillPerLevel))
            {
                diff[$"LevelUpData[{i}].SkillPerLevel"] = modData.SkillPerLevel;
            }
        }

        return diff;
    }
}
