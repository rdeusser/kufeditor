using System;
using System.IO;
using System.Text;
using KUFEditor.Core.FileFormats;

namespace KUFEditor.Assets.TroopInfo;

/// <summary>
/// Handles reading and writing of TroopInfo.sox files.
/// </summary>
public class TroopInfoSoxFile : IFileFormat
{
    public string Extension => ".sox";
    public string Description => "TroopInfo SOX File";

    public bool CanRead(string path)
    {
        return Path.GetFileName(path).Equals("TroopInfo.sox", StringComparison.OrdinalIgnoreCase);
    }

    public bool CanWrite(string path)
    {
        return CanRead(path);
    }

    public object Read(string path)
    {
        using var fs = File.OpenRead(path);
        using var reader = new BinaryReader(fs, Encoding.UTF8, false);

        var sox = new TroopInfoSox
        {
            Version = reader.ReadInt32(),
            Count = reader.ReadInt32()
        };

        if (!sox.IsValid())
        {
            throw new InvalidDataException(
                $"Invalid TroopInfo.sox file: version={sox.Version}, count={sox.Count}. " +
                $"Expected version={TroopInfoSox.VALID_VERSION}, count={TroopInfoSox.TROOP_COUNT}");
        }

        // Read all 43 troops
        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            sox.TroopInfos[i] = ReadTroopInfo(reader);
        }

        // Read the 64-byte padding at the end
        sox.TheEnd = reader.ReadBytes(TroopInfoSox.PADDING_SIZE);

        return sox;
    }

    public void Write(string path, object data)
    {
        if (data is not TroopInfoSox sox)
        {
            throw new ArgumentException("Data must be of type TroopInfoSox", nameof(data));
        }

        // Create backup if file exists
        if (File.Exists(path))
        {
            string backupPath = path + ".bak";
            File.Copy(path, backupPath, true);
        }

        using var fs = File.Create(path);
        using var writer = new BinaryWriter(fs, Encoding.UTF8, false);

        // Write header
        writer.Write(sox.Version);
        writer.Write(sox.Count);

        // Write all 43 troops
        for (int i = 0; i < TroopInfoSox.TROOP_COUNT; i++)
        {
            WriteTroopInfo(writer, sox.TroopInfos[i]);
        }

        // Write padding
        writer.Write(sox.TheEnd);
    }

    private TroopInfo ReadTroopInfo(BinaryReader reader)
    {
        var troop = new TroopInfo
        {
            Job = reader.ReadInt32(),
            TypeID = reader.ReadInt32(),

            MoveSpeed = reader.ReadSingle(),
            RotateRate = reader.ReadSingle(),
            MoveAcceleration = reader.ReadSingle(),
            MoveDeceleration = reader.ReadSingle(),

            SightRange = reader.ReadSingle(),

            AttackRangeMax = reader.ReadSingle(),
            AttackRangeMin = reader.ReadSingle(),
            AttackFrontRange = reader.ReadSingle(),

            DirectAttack = reader.ReadSingle(),
            IndirectAttack = reader.ReadSingle(),
            Defense = reader.ReadSingle(),

            BaseWidth = reader.ReadSingle(),

            ResistMelee = reader.ReadSingle(),
            ResistRanged = reader.ReadSingle(),
            ResistFrontal = reader.ReadSingle(),
            ResistExplosion = reader.ReadSingle(),
            ResistFire = reader.ReadSingle(),
            ResistIce = reader.ReadSingle(),
            ResistLightning = reader.ReadSingle(),
            ResistHoly = reader.ReadSingle(),
            ResistCurse = reader.ReadSingle(),
            ResistPoison = reader.ReadSingle(),

            MaxUnitSpeedMultiplier = reader.ReadSingle(),
            DefaultUnitHP = reader.ReadSingle(),
            FormationRandom = reader.ReadInt32(),
            DefaultUnitNumX = reader.ReadInt32(),
            DefaultUnitNumY = reader.ReadInt32(),

            UnitHPLevUp = reader.ReadSingle()
        };

        // Read 3 level up data entries
        for (int i = 0; i < 3; i++)
        {
            troop.LevelUpData[i] = new LevelUpData
            {
                SkillID = reader.ReadInt32(),
                SkillPerLevel = reader.ReadSingle()
            };
        }

        troop.DamageDistribution = reader.ReadSingle();

        return troop;
    }

    private void WriteTroopInfo(BinaryWriter writer, TroopInfo troop)
    {
        writer.Write(troop.Job);
        writer.Write(troop.TypeID);

        writer.Write(troop.MoveSpeed);
        writer.Write(troop.RotateRate);
        writer.Write(troop.MoveAcceleration);
        writer.Write(troop.MoveDeceleration);

        writer.Write(troop.SightRange);

        writer.Write(troop.AttackRangeMax);
        writer.Write(troop.AttackRangeMin);
        writer.Write(troop.AttackFrontRange);

        writer.Write(troop.DirectAttack);
        writer.Write(troop.IndirectAttack);
        writer.Write(troop.Defense);

        writer.Write(troop.BaseWidth);

        writer.Write(troop.ResistMelee);
        writer.Write(troop.ResistRanged);
        writer.Write(troop.ResistFrontal);
        writer.Write(troop.ResistExplosion);
        writer.Write(troop.ResistFire);
        writer.Write(troop.ResistIce);
        writer.Write(troop.ResistLightning);
        writer.Write(troop.ResistHoly);
        writer.Write(troop.ResistCurse);
        writer.Write(troop.ResistPoison);

        writer.Write(troop.MaxUnitSpeedMultiplier);
        writer.Write(troop.DefaultUnitHP);
        writer.Write(troop.FormationRandom);
        writer.Write(troop.DefaultUnitNumX);
        writer.Write(troop.DefaultUnitNumY);

        writer.Write(troop.UnitHPLevUp);

        // Write 3 level up data entries
        for (int i = 0; i < 3; i++)
        {
            writer.Write(troop.LevelUpData[i].SkillID);
            writer.Write(troop.LevelUpData[i].SkillPerLevel);
        }

        writer.Write(troop.DamageDistribution);
    }

    public static void RestoreFromBackup(string path)
    {
        string backupPath = path + ".bak";
        if (File.Exists(backupPath))
        {
            File.Copy(backupPath, path, true);
        }
        else
        {
            throw new FileNotFoundException("Backup file not found", backupPath);
        }
    }
}