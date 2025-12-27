namespace KUFEditor.Assets.TroopInfo;

/// <summary>
/// Represents a troop's data in Kingdom Under Fire.
/// </summary>
public class TroopInfo
{
    public int Job { get; set; } // troop Job type (defined in K2JobDef.h)
    public int TypeID { get; set; } // troop type ID (defined in K2TroopDef.h)

    // Movement properties
    public float MoveSpeed { get; set; } // max move speed
    public float RotateRate { get; set; } // max rotate rate
    public float MoveAcceleration { get; set; } // move acceleration
    public float MoveDeceleration { get; set; } // move deceleration

    // Vision
    public float SightRange { get; set; } // visible range

    // Attack ranges
    public float AttackRangeMax { get; set; }
    public float AttackRangeMin { get; set; } // ranged attack range (0 if troop lacks ranged attack)
    public float AttackFrontRange { get; set; } // frontal attack range (0 if troop lacks frontal attack)

    // Combat stats
    public float DirectAttack { get; set; } // direct attack strength (melee/frontal)
    public float IndirectAttack { get; set; } // indirect attack strength (ranged)
    public float Defense { get; set; } // defense strength

    public float BaseWidth { get; set; } // base troop size

    // Resistance to attack types
    public float ResistMelee { get; set; }
    public float ResistRanged { get; set; }
    public float ResistFrontal { get; set; }
    public float ResistExplosion { get; set; }
    public float ResistFire { get; set; }
    public float ResistIce { get; set; }
    public float ResistLightning { get; set; }
    public float ResistHoly { get; set; }
    public float ResistCurse { get; set; }
    public float ResistPoison { get; set; }

    // Unit configuration
    public float MaxUnitSpeedMultiplier { get; set; }
    public float DefaultUnitHP { get; set; }
    public int FormationRandom { get; set; }
    public int DefaultUnitNumX { get; set; }
    public int DefaultUnitNumY { get; set; }

    public float UnitHPLevUp { get; set; }

    // Fixed size array of 3 level up data entries
    public LevelUpData[] LevelUpData { get; set; } = new LevelUpData[3];

    public float DamageDistribution { get; set; }

    public TroopInfo()
    {
        // Initialize LevelUpData array
        for (int i = 0; i < 3; i++)
        {
            LevelUpData[i] = new LevelUpData();
        }
    }
}