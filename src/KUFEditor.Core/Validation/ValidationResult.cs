namespace KUFEditor.Core.Validation;

/// <summary>
/// Represents the result of a file validation.
/// </summary>
public class ValidationResult
{
    private readonly List<ValidationIssue> _issues = new();

    /// <summary>
    /// Gets the file path that was validated.
    /// </summary>
    public string FilePath { get; }

    /// <summary>
    /// Gets all validation issues found.
    /// </summary>
    public IReadOnlyList<ValidationIssue> Issues => _issues;

    /// <summary>
    /// Gets whether the validation passed (no errors).
    /// </summary>
    public bool IsValid => !_issues.Any(i => i.Severity == IssueSeverity.Error);

    /// <summary>
    /// Gets whether there are any warnings.
    /// </summary>
    public bool HasWarnings => _issues.Any(i => i.Severity == IssueSeverity.Warning);

    /// <summary>
    /// Gets the count of errors.
    /// </summary>
    public int ErrorCount => _issues.Count(i => i.Severity == IssueSeverity.Error);

    /// <summary>
    /// Gets the count of warnings.
    /// </summary>
    public int WarningCount => _issues.Count(i => i.Severity == IssueSeverity.Warning);

    public ValidationResult(string filePath)
    {
        FilePath = filePath ?? throw new ArgumentNullException(nameof(filePath));
    }

    /// <summary>
    /// Adds an error to the result.
    /// </summary>
    public void AddError(string code, string message, long? position = null)
    {
        _issues.Add(new ValidationIssue(IssueSeverity.Error, code, message, position));
    }

    /// <summary>
    /// Adds a warning to the result.
    /// </summary>
    public void AddWarning(string code, string message, long? position = null)
    {
        _issues.Add(new ValidationIssue(IssueSeverity.Warning, code, message, position));
    }

    /// <summary>
    /// Adds an info message to the result.
    /// </summary>
    public void AddInfo(string code, string message, long? position = null)
    {
        _issues.Add(new ValidationIssue(IssueSeverity.Info, code, message, position));
    }

    /// <summary>
    /// Merges another result into this one.
    /// </summary>
    public void Merge(ValidationResult other)
    {
        foreach (var issue in other.Issues)
        {
            _issues.Add(issue);
        }
    }
}

/// <summary>
/// Represents a single validation issue.
/// </summary>
public class ValidationIssue
{
    /// <summary>
    /// Gets the severity of the issue.
    /// </summary>
    public IssueSeverity Severity { get; }

    /// <summary>
    /// Gets the error code.
    /// </summary>
    public string Code { get; }

    /// <summary>
    /// Gets the message describing the issue.
    /// </summary>
    public string Message { get; }

    /// <summary>
    /// Gets the byte position in the file where the issue was found, if applicable.
    /// </summary>
    public long? Position { get; }

    public ValidationIssue(IssueSeverity severity, string code, string message, long? position = null)
    {
        Severity = severity;
        Code = code ?? throw new ArgumentNullException(nameof(code));
        Message = message ?? throw new ArgumentNullException(nameof(message));
        Position = position;
    }

    public override string ToString()
    {
        var pos = Position.HasValue ? $" at position {Position}" : "";
        return $"[{Severity}] {Code}: {Message}{pos}";
    }
}

/// <summary>
/// Severity levels for validation issues.
/// </summary>
public enum IssueSeverity
{
    Info,
    Warning,
    Error
}
