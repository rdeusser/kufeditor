using Xunit;
using KUFEditor.Core.Validation;

namespace KUFEditor.Core.Tests;

public class ValidationResultTests
{
    [Fact]
    public void Constructor_SetsFilePath()
    {
        var result = new ValidationResult("/path/to/file.sox");

        Assert.Equal("/path/to/file.sox", result.FilePath);
    }

    [Fact]
    public void IsValid_TrueWhenNoErrors()
    {
        var result = new ValidationResult("/test.sox");

        Assert.True(result.IsValid);
    }

    [Fact]
    public void IsValid_FalseWhenHasErrors()
    {
        var result = new ValidationResult("/test.sox");
        result.AddError("ERR001", "Test error");

        Assert.False(result.IsValid);
    }

    [Fact]
    public void IsValid_TrueWithOnlyWarnings()
    {
        var result = new ValidationResult("/test.sox");
        result.AddWarning("WARN001", "Test warning");

        Assert.True(result.IsValid);
        Assert.True(result.HasWarnings);
    }

    [Fact]
    public void AddError_IncreasesErrorCount()
    {
        var result = new ValidationResult("/test.sox");

        result.AddError("ERR001", "Error 1");
        result.AddError("ERR002", "Error 2");

        Assert.Equal(2, result.ErrorCount);
    }

    [Fact]
    public void AddWarning_IncreasesWarningCount()
    {
        var result = new ValidationResult("/test.sox");

        result.AddWarning("WARN001", "Warning 1");
        result.AddWarning("WARN002", "Warning 2");

        Assert.Equal(2, result.WarningCount);
    }

    [Fact]
    public void AddError_WithPosition_StoresPosition()
    {
        var result = new ValidationResult("/test.sox");

        result.AddError("ERR001", "Error at position", 100);

        Assert.Equal(100, result.Issues[0].Position);
    }

    [Fact]
    public void Merge_CombinesResults()
    {
        var result1 = new ValidationResult("/test.sox");
        result1.AddError("ERR001", "Error 1");

        var result2 = new ValidationResult("/test.sox");
        result2.AddWarning("WARN001", "Warning 1");

        result1.Merge(result2);

        Assert.Equal(1, result1.ErrorCount);
        Assert.Equal(1, result1.WarningCount);
    }
}

public class ValidationIssueTests
{
    [Fact]
    public void Constructor_SetsProperties()
    {
        var issue = new ValidationIssue(IssueSeverity.Error, "ERR001", "Test message", 50);

        Assert.Equal(IssueSeverity.Error, issue.Severity);
        Assert.Equal("ERR001", issue.Code);
        Assert.Equal("Test message", issue.Message);
        Assert.Equal(50, issue.Position);
    }

    [Fact]
    public void ToString_FormatsCorrectly()
    {
        var issue = new ValidationIssue(IssueSeverity.Error, "ERR001", "Test message", 50);

        var str = issue.ToString();

        Assert.Contains("[Error]", str);
        Assert.Contains("ERR001", str);
        Assert.Contains("Test message", str);
        Assert.Contains("50", str);
    }

    [Fact]
    public void ToString_WithoutPosition_OmitsPosition()
    {
        var issue = new ValidationIssue(IssueSeverity.Warning, "WARN001", "Test message");

        var str = issue.ToString();

        Assert.DoesNotContain("position", str);
    }
}

public class SoxFileValidatorTests
{
    [Fact]
    public void CanValidate_TroopInfoSox_ReturnsTrue()
    {
        var validator = new SoxFileValidator();

        Assert.True(validator.CanValidate("/game/TroopInfo.sox"));
    }

    [Fact]
    public void CanValidate_SkillInfoSox_ReturnsTrue()
    {
        var validator = new SoxFileValidator();

        Assert.True(validator.CanValidate("/game/SkillInfo.sox"));
    }

    [Fact]
    public void CanValidate_ExpInfoSox_ReturnsTrue()
    {
        var validator = new SoxFileValidator();

        Assert.True(validator.CanValidate("/game/ExpInfo.sox"));
    }

    [Fact]
    public void CanValidate_RandomFile_ReturnsFalse()
    {
        var validator = new SoxFileValidator();

        Assert.False(validator.CanValidate("/game/random.sox"));
    }

    [Fact]
    public void Validate_NonExistentFile_ReturnsError()
    {
        var validator = new SoxFileValidator();

        var result = validator.Validate("/nonexistent/TroopInfo.sox");

        Assert.False(result.IsValid);
        Assert.Equal(SoxFileValidator.ErrorCodes.FileNotFound, result.Issues[0].Code);
    }

    [Fact]
    public void Validate_TooSmallFile_ReturnsError()
    {
        var validator = new SoxFileValidator();
        using var stream = new MemoryStream(new byte[4]);

        var result = validator.Validate(stream, "TroopInfo.sox");

        Assert.False(result.IsValid);
        Assert.Equal(SoxFileValidator.ErrorCodes.FileTooSmall, result.Issues[0].Code);
    }

    [Fact]
    public void Validate_InvalidVersion_ReturnsError()
    {
        var validator = new SoxFileValidator();
        var data = CreateTroopInfoHeader(99, 43);
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "TroopInfo.sox");

        Assert.False(result.IsValid);
        Assert.Contains(result.Issues, i => i.Code == SoxFileValidator.ErrorCodes.InvalidVersion);
    }

    [Fact]
    public void Validate_InvalidCount_ReturnsError()
    {
        var validator = new SoxFileValidator();
        var data = CreateTroopInfoHeader(100, 10);
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "TroopInfo.sox");

        Assert.False(result.IsValid);
        Assert.Contains(result.Issues, i => i.Code == SoxFileValidator.ErrorCodes.InvalidCount);
    }

    [Fact]
    public void Validate_ValidHeader_PassesHeaderCheck()
    {
        var validator = new SoxFileValidator();
        var data = CreateTroopInfoHeader(100, 43);
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "TroopInfo.sox");

        Assert.DoesNotContain(result.Issues,
            i => i.Code == SoxFileValidator.ErrorCodes.InvalidVersion ||
                 i.Code == SoxFileValidator.ErrorCodes.InvalidCount);
    }

    [Fact]
    public void SupportedExtensions_ContainsSox()
    {
        var validator = new SoxFileValidator();

        Assert.Contains(".sox", validator.SupportedExtensions);
    }

    private static byte[] CreateTroopInfoHeader(int version, int count)
    {
        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms);

        writer.Write(version);
        writer.Write(count);

        return ms.ToArray();
    }
}

public class TextSoxValidatorTests
{
    [Fact]
    public void CanValidate_ItemTypeInfo_ReturnsTrue()
    {
        var validator = new TextSoxValidator();

        Assert.True(validator.CanValidate("/game/ItemTypeInfo_ENG.sox"));
    }

    [Fact]
    public void CanValidate_RegularSox_ReturnsFalse()
    {
        var validator = new TextSoxValidator();

        Assert.False(validator.CanValidate("/game/TroopInfo.sox"));
    }

    [Fact]
    public void Validate_TooSmallFile_ReturnsError()
    {
        var validator = new TextSoxValidator();
        using var stream = new MemoryStream(new byte[1]);

        var result = validator.Validate(stream, "ItemTypeInfo_ENG.sox");

        Assert.False(result.IsValid);
        Assert.Equal(TextSoxValidator.ErrorCodes.FileTooSmall, result.Issues[0].Code);
    }

    [Fact]
    public void Validate_ValidEntries_ReturnsValid()
    {
        var validator = new TextSoxValidator();
        var data = CreateTextSoxData(new[] { "Hello", "World" });
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "ItemTypeInfo_ENG.sox");

        Assert.True(result.IsValid);
    }

    [Fact]
    public void Validate_CountsEntries()
    {
        var validator = new TextSoxValidator();
        var data = CreateTextSoxData(new[] { "One", "Two", "Three" });
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "ItemTypeInfo_ENG.sox");

        Assert.Contains(result.Issues, i => i.Message.Contains("3 entries"));
    }

    [Fact]
    public void Validate_EmptyEntries_ReportsWarning()
    {
        var validator = new TextSoxValidator();
        var data = CreateTextSoxDataWithEmptyEntry();
        using var stream = new MemoryStream(data);

        var result = validator.Validate(stream, "ItemTypeInfo_ENG.sox");

        Assert.Contains(result.Issues, i => i.Code == TextSoxValidator.ErrorCodes.EmptyEntry);
    }

    [Fact]
    public void Validate_TruncatedEntry_ReportsError()
    {
        var validator = new TextSoxValidator();
        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms);

        writer.Write((short)100); // claims 100 bytes but file ends
        ms.Position = 0;

        var result = validator.Validate(ms, "ItemTypeInfo_ENG.sox");

        Assert.Contains(result.Issues, i => i.Code == TextSoxValidator.ErrorCodes.TruncatedEntry);
    }

    private static byte[] CreateTextSoxData(string[] entries)
    {
        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms);

        foreach (var entry in entries)
        {
            var bytes = System.Text.Encoding.UTF8.GetBytes(entry);
            writer.Write((short)bytes.Length);
            writer.Write(bytes);
        }

        return ms.ToArray();
    }

    private static byte[] CreateTextSoxDataWithEmptyEntry()
    {
        using var ms = new MemoryStream();
        using var writer = new BinaryWriter(ms);

        writer.Write((short)5);
        writer.Write(System.Text.Encoding.UTF8.GetBytes("Hello"));
        writer.Write((short)0); // empty entry
        writer.Write((short)5);
        writer.Write(System.Text.Encoding.UTF8.GetBytes("World"));

        return ms.ToArray();
    }
}

public class ValidationServiceTests
{
    [Fact]
    public void RegisterValidator_AddsToList()
    {
        var service = new ValidationService();
        var validator = new SoxFileValidator();

        service.RegisterValidator(validator);

        Assert.Contains(validator, service.Validators);
    }

    [Fact]
    public void RegisterDefaults_AddsDefaultValidators()
    {
        var service = new ValidationService();

        service.RegisterDefaults();

        Assert.Equal(2, service.Validators.Count);
    }

    [Fact]
    public void ValidateFile_RaisesEvents()
    {
        var service = new ValidationService();
        service.RegisterDefaults();

        string? startedPath = null;
        ValidationResult? completedResult = null;

        service.ValidationStarted += (s, p) => startedPath = p;
        service.ValidationCompleted += (s, r) => completedResult = r;

        service.ValidateFile("/nonexistent/TroopInfo.sox");

        Assert.Equal("/nonexistent/TroopInfo.sox", startedPath);
        Assert.NotNull(completedResult);
    }

    [Fact]
    public void ValidateFile_NoValidator_ReturnsInfo()
    {
        var service = new ValidationService();

        var result = service.ValidateFile("/random/file.txt");

        Assert.True(result.IsValid);
        Assert.Contains(result.Issues, i => i.Code == "VAL001");
    }
}

public class ValidationSummaryTests
{
    [Fact]
    public void Constructor_SetsRootPath()
    {
        var summary = new ValidationSummary("/game");

        Assert.Equal("/game", summary.RootPath);
    }

    [Fact]
    public void AddResult_IncreasesTotalFiles()
    {
        var summary = new ValidationSummary("/game");

        summary.AddResult(new ValidationResult("/file1.sox"));
        summary.AddResult(new ValidationResult("/file2.sox"));

        Assert.Equal(2, summary.TotalFiles);
    }

    [Fact]
    public void FilesWithErrors_CountsCorrectly()
    {
        var summary = new ValidationSummary("/game");

        var valid = new ValidationResult("/valid.sox");
        var invalid = new ValidationResult("/invalid.sox");
        invalid.AddError("ERR001", "Error");

        summary.AddResult(valid);
        summary.AddResult(invalid);

        Assert.Equal(1, summary.FilesWithErrors);
    }

    [Fact]
    public void TotalErrors_SumsAcrossFiles()
    {
        var summary = new ValidationSummary("/game");

        var result1 = new ValidationResult("/file1.sox");
        result1.AddError("ERR001", "Error 1");
        result1.AddError("ERR002", "Error 2");

        var result2 = new ValidationResult("/file2.sox");
        result2.AddError("ERR003", "Error 3");

        summary.AddResult(result1);
        summary.AddResult(result2);

        Assert.Equal(3, summary.TotalErrors);
    }

    [Fact]
    public void AllValid_TrueWhenNoErrors()
    {
        var summary = new ValidationSummary("/game");

        var result = new ValidationResult("/file.sox");
        result.AddWarning("WARN001", "Warning");

        summary.AddResult(result);

        Assert.True(summary.AllValid);
    }

    [Fact]
    public void AllValid_FalseWhenHasErrors()
    {
        var summary = new ValidationSummary("/game");

        var result = new ValidationResult("/file.sox");
        result.AddError("ERR001", "Error");

        summary.AddResult(result);

        Assert.False(summary.AllValid);
    }

    [Fact]
    public void GetErrorResults_ReturnsOnlyErrors()
    {
        var summary = new ValidationSummary("/game");

        var valid = new ValidationResult("/valid.sox");
        var invalid = new ValidationResult("/invalid.sox");
        invalid.AddError("ERR001", "Error");

        summary.AddResult(valid);
        summary.AddResult(invalid);

        var errors = summary.GetErrorResults().ToList();

        Assert.Single(errors);
        Assert.Equal("/invalid.sox", errors[0].FilePath);
    }
}
