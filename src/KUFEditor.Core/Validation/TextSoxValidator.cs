using System.Text;

namespace KUFEditor.Core.Validation;

/// <summary>
/// Validates text-based SOX files like ItemTypeInfo_ENG.sox.
/// </summary>
public class TextSoxValidator : IFileValidator
{
    /// <summary>
    /// Error codes for text SOX validation.
    /// </summary>
    public static class ErrorCodes
    {
        public const string FileNotFound = "TSOX001";
        public const string FileTooSmall = "TSOX002";
        public const string InvalidEncoding = "TSOX003";
        public const string NoEntries = "TSOX004";
        public const string TruncatedEntry = "TSOX005";
        public const string ControlCharacters = "TSOX006";
        public const string EmptyEntry = "TSOX007";
    }

    private static readonly string[] TextSoxPatterns =
    {
        "ItemTypeInfo",
        "CharName",
        "ToolTip",
        "Message",
        "Text",
        "_ENG.sox",
        "_KOR.sox",
        "_JPN.sox"
    };

    // Entry structure: 2-byte length prefix + content.
    private const int MinEntrySize = 2;

    public IEnumerable<string> SupportedExtensions => new[] { ".sox" };

    public bool CanValidate(string path)
    {
        var fileName = Path.GetFileName(path);
        return TextSoxPatterns.Any(p => fileName.Contains(p, StringComparison.OrdinalIgnoreCase));
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

        if (stream.Length < MinEntrySize)
        {
            result.AddError(ErrorCodes.FileTooSmall, "File too small to contain any entries");
            return result;
        }

        using var reader = new BinaryReader(stream, Encoding.UTF8, leaveOpen: true);

        var entryCount = 0;
        var emptyEntries = 0;
        var truncatedEntries = 0;
        var entriesWithControlChars = 0;

        while (stream.Position < stream.Length - 1)
        {
            var entryStart = stream.Position;

            try
            {
                var length = reader.ReadInt16();

                if (length < 0)
                {
                    result.AddError(ErrorCodes.TruncatedEntry,
                        $"Invalid entry length: {length}", entryStart);
                    break;
                }

                if (length == 0)
                {
                    emptyEntries++;
                    entryCount++;
                    continue;
                }

                if (stream.Position + length > stream.Length)
                {
                    result.AddError(ErrorCodes.TruncatedEntry,
                        $"Entry {entryCount}: Length {length} exceeds file boundary", entryStart);
                    truncatedEntries++;
                    break;
                }

                var content = reader.ReadBytes(length);

                // Check for control characters (excluding newlines and tabs).
                if (ContainsUnexpectedControlChars(content))
                {
                    entriesWithControlChars++;
                }

                entryCount++;
            }
            catch (EndOfStreamException)
            {
                result.AddError(ErrorCodes.TruncatedEntry,
                    $"Unexpected end of file at entry {entryCount}", entryStart);
                break;
            }
        }

        if (entryCount == 0)
        {
            result.AddError(ErrorCodes.NoEntries, "No valid entries found in file");
        }
        else
        {
            result.AddInfo("TSOX010", $"Found {entryCount} entries");

            if (emptyEntries > 0)
            {
                result.AddWarning(ErrorCodes.EmptyEntry, $"{emptyEntries} empty entries found");
            }

            if (entriesWithControlChars > 0)
            {
                result.AddWarning(ErrorCodes.ControlCharacters,
                    $"{entriesWithControlChars} entries contain control characters");
            }
        }

        return result;
    }

    private static bool ContainsUnexpectedControlChars(byte[] content)
    {
        foreach (var b in content)
        {
            // Allow tab (9), newline (10), carriage return (13).
            if (b < 32 && b != 9 && b != 10 && b != 13)
            {
                return true;
            }
        }
        return false;
    }
}
