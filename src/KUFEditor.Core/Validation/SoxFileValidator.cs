namespace KUFEditor.Core.Validation;

/// <summary>
/// Validates SOX file structure and integrity.
/// </summary>
public class SoxFileValidator : IFileValidator
{
    /// <summary>
    /// Error codes for SOX validation.
    /// </summary>
    public static class ErrorCodes
    {
        public const string FileNotFound = "SOX001";
        public const string FileTooSmall = "SOX002";
        public const string InvalidVersion = "SOX003";
        public const string InvalidCount = "SOX004";
        public const string UnexpectedFileSize = "SOX005";
        public const string InvalidFloatValue = "SOX006";
        public const string UnexpectedEof = "SOX007";
        public const string InvalidHeader = "SOX008";
    }

    private static readonly string[] SupportedFileNames =
    {
        "TroopInfo.sox",
        "SkillInfo.sox",
        "ExpInfo.sox"
    };

    // TroopInfo.sox constants.
    private const int TroopInfoVersion = 100;
    private const int TroopInfoCount = 43;
    private const int TroopInfoRecordSize = 136;
    private const int TroopInfoPaddingSize = 64;

    public IEnumerable<string> SupportedExtensions => new[] { ".sox" };

    public bool CanValidate(string path)
    {
        var fileName = Path.GetFileName(path);
        return SupportedFileNames.Any(n => n.Equals(fileName, StringComparison.OrdinalIgnoreCase));
    }

    public ValidationResult Validate(string path)
    {
        var result = new ValidationResult(path);

        if (!File.Exists(path))
        {
            result.AddError(ErrorCodes.FileNotFound, $"File not found: {path}");
            return result;
        }

        using var stream = File.OpenRead(path);
        return Validate(stream, path);
    }

    public ValidationResult Validate(Stream stream, string filePath)
    {
        var result = new ValidationResult(filePath);
        var fileName = Path.GetFileName(filePath);

        if (fileName.Equals("TroopInfo.sox", StringComparison.OrdinalIgnoreCase))
        {
            ValidateTroopInfo(stream, result);
        }
        else if (fileName.Equals("SkillInfo.sox", StringComparison.OrdinalIgnoreCase))
        {
            ValidateSkillInfo(stream, result);
        }
        else if (fileName.Equals("ExpInfo.sox", StringComparison.OrdinalIgnoreCase))
        {
            ValidateExpInfo(stream, result);
        }
        else
        {
            ValidateGenericBinarySox(stream, result);
        }

        return result;
    }

    private void ValidateTroopInfo(Stream stream, ValidationResult result)
    {
        var expectedSize = 8 + (TroopInfoCount * TroopInfoRecordSize) + TroopInfoPaddingSize;

        if (stream.Length < 8)
        {
            result.AddError(ErrorCodes.FileTooSmall, "File too small to contain header (minimum 8 bytes)");
            return;
        }

        using var reader = new BinaryReader(stream, System.Text.Encoding.UTF8, leaveOpen: true);

        var version = reader.ReadInt32();
        var count = reader.ReadInt32();

        if (version != TroopInfoVersion)
        {
            result.AddError(ErrorCodes.InvalidVersion, $"Invalid version: {version}. Expected: {TroopInfoVersion}", 0);
        }

        if (count != TroopInfoCount)
        {
            result.AddError(ErrorCodes.InvalidCount, $"Invalid count: {count}. Expected: {TroopInfoCount}", 4);
        }

        if (stream.Length != expectedSize)
        {
            result.AddWarning(ErrorCodes.UnexpectedFileSize,
                $"Unexpected file size: {stream.Length} bytes. Expected: {expectedSize} bytes");
        }

        // Validate each troop record.
        for (int i = 0; i < TroopInfoCount && stream.Position < stream.Length - TroopInfoPaddingSize; i++)
        {
            ValidateTroopRecord(reader, result, i);
        }
    }

    private void ValidateTroopRecord(BinaryReader reader, ValidationResult result, int index)
    {
        var recordStart = reader.BaseStream.Position;

        try
        {
            var job = reader.ReadInt32();
            var typeId = reader.ReadInt32();

            // Validate float values.
            var floatFields = new[] { "MoveSpeed", "RotateRate", "MoveAcceleration", "MoveDeceleration",
                "SightRange", "AttackRangeMax", "AttackRangeMin", "AttackFrontRange",
                "DirectAttack", "IndirectAttack", "Defense", "BaseWidth",
                "ResistMelee", "ResistRanged", "ResistFrontal", "ResistExplosion",
                "ResistFire", "ResistIce", "ResistLightning", "ResistHoly",
                "ResistCurse", "ResistPoison", "MaxUnitSpeedMultiplier", "DefaultUnitHP" };

            foreach (var field in floatFields)
            {
                var value = reader.ReadSingle();
                if (float.IsNaN(value) || float.IsInfinity(value))
                {
                    result.AddWarning(ErrorCodes.InvalidFloatValue,
                        $"Troop {index}: {field} has invalid float value: {value}", recordStart);
                }
            }

            // Read remaining fields.
            reader.ReadInt32(); // FormationRandom
            reader.ReadInt32(); // DefaultUnitNumX
            reader.ReadInt32(); // DefaultUnitNumY
            reader.ReadSingle(); // UnitHPLevUp

            // 3 level up entries.
            for (int j = 0; j < 3; j++)
            {
                reader.ReadInt32(); // SkillID
                var levelValue = reader.ReadSingle();
                if (float.IsNaN(levelValue) || float.IsInfinity(levelValue))
                {
                    result.AddWarning(ErrorCodes.InvalidFloatValue,
                        $"Troop {index}: LevelUp[{j}].SkillPerLevel has invalid value", recordStart);
                }
            }

            reader.ReadSingle(); // DamageDistribution
        }
        catch (EndOfStreamException)
        {
            result.AddError(ErrorCodes.UnexpectedEof, $"Unexpected end of file reading troop {index}", recordStart);
        }
    }

    private void ValidateSkillInfo(Stream stream, ValidationResult result)
    {
        if (stream.Length < 4)
        {
            result.AddError(ErrorCodes.FileTooSmall, "File too small to contain header");
            return;
        }

        using var reader = new BinaryReader(stream, System.Text.Encoding.UTF8, leaveOpen: true);

        var magic = reader.ReadInt32();
        if (magic < 0 || magic > 10000)
        {
            result.AddWarning(ErrorCodes.InvalidHeader, $"Suspicious header value: {magic}");
        }

        result.AddInfo("SKILL001", $"SkillInfo.sox: Header value {magic}, file size {stream.Length} bytes");
    }

    private void ValidateExpInfo(Stream stream, ValidationResult result)
    {
        if (stream.Length < 4)
        {
            result.AddError(ErrorCodes.FileTooSmall, "File too small to contain header");
            return;
        }

        result.AddInfo("EXP001", $"ExpInfo.sox: File size {stream.Length} bytes");
    }

    private void ValidateGenericBinarySox(Stream stream, ValidationResult result)
    {
        if (stream.Length < 4)
        {
            result.AddError(ErrorCodes.FileTooSmall, "File too small to be a valid SOX file");
            return;
        }

        result.AddInfo("GENERIC001", $"Generic SOX file: {stream.Length} bytes");
    }
}
