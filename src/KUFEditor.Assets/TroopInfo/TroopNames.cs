namespace KUFEditor.Assets.TroopInfo;

/// <summary>
/// Contains the names of all 43 troops in Kingdom Under Fire.
/// </summary>
public static class TroopNames
{
    public static readonly string[] Names = new string[]
    {
        "Archer",
        "Longbows",
        "Infantry",
        "Spearman",
        "Heavy Infantry",
        "Knight",
        "Paladin",
        "Calvary",
        "Heavy Calvary",
        "Storm Riders",
        "Sappers",
        "Pyro Techs",
        "Bomber Wings",
        "Mortar",
        "Ballista",
        "Harpoon",
        "Catapult",
        "Battaloon",
        "Dark Elves Archer",
        "Dark Elves Calvary Archers",
        "Dark Elves Infantry",
        "Dark Elves Knights",
        "Dark Elves Calvary",
        "Orc Infantry",
        "Orc Riders",
        "Orc Heavy Riders",
        "Orc Axe Man",
        "Orc Heavy Infantry",
        "Orc Sappers",
        "Orc Scorpion",
        "Orc Swamp Mammoth",
        "Orc Dirigible",
        "Orc Black Wyverns",
        "Orc Ghouls",
        "Orc Bone Dragon",
        "Wall Archers (Humans)",
        "Scouts",
        "Ghoul Selfdestruct",
        "Encablossa Monster (Melee)",
        "Encablossa Flying Monster",
        "Encablossa Monster (Ranged)",
        "Wall Archers (Elves)",
        "Encablossa Main"
    };

    public static string GetName(int index)
    {
        if (index >= 0 && index < Names.Length)
            return Names[index];
        return $"Unknown Troop {index}";
    }
}