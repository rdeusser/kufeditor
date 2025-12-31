namespace KUFEditor.Core.Validation;

/// <summary>
/// Service for validating game files.
/// </summary>
public class ValidationService
{
    private readonly List<IFileValidator> _validators = new();

    /// <summary>
    /// Raised when validation starts for a file.
    /// </summary>
    public event EventHandler<string>? ValidationStarted;

    /// <summary>
    /// Raised when validation completes for a file.
    /// </summary>
    public event EventHandler<ValidationResult>? ValidationCompleted;

    /// <summary>
    /// Gets the registered validators.
    /// </summary>
    public IReadOnlyList<IFileValidator> Validators => _validators;

    /// <summary>
    /// Registers a validator.
    /// </summary>
    public void RegisterValidator(IFileValidator validator)
    {
        ArgumentNullException.ThrowIfNull(validator);
        _validators.Add(validator);
    }

    /// <summary>
    /// Registers the default validators.
    /// </summary>
    public void RegisterDefaults()
    {
        _validators.Add(new SoxFileValidator());
        _validators.Add(new TextSoxValidator());
    }

    /// <summary>
    /// Validates a single file.
    /// </summary>
    public ValidationResult ValidateFile(string path)
    {
        ValidationStarted?.Invoke(this, path);

        var validator = _validators.FirstOrDefault(v => v.CanValidate(path));

        ValidationResult result;
        if (validator == null)
        {
            result = new ValidationResult(path);
            result.AddInfo("VAL001", "No validator available for this file type");
        }
        else
        {
            result = validator.Validate(path);
        }

        ValidationCompleted?.Invoke(this, result);
        return result;
    }

    /// <summary>
    /// Validates all supported files in a directory.
    /// </summary>
    public IEnumerable<ValidationResult> ValidateDirectory(string path, bool recursive = true)
    {
        var searchOption = recursive ? SearchOption.AllDirectories : SearchOption.TopDirectoryOnly;
        var results = new List<ValidationResult>();

        foreach (var file in Directory.EnumerateFiles(path, "*.*", searchOption))
        {
            var validator = _validators.FirstOrDefault(v => v.CanValidate(file));
            if (validator != null)
            {
                results.Add(ValidateFile(file));
            }
        }

        return results;
    }

    /// <summary>
    /// Validates all SOX files in a game directory.
    /// </summary>
    public ValidationSummary ValidateGameFiles(string gamePath)
    {
        var summary = new ValidationSummary(gamePath);
        var soxFiles = new List<string>();

        // Find all SOX files.
        if (Directory.Exists(gamePath))
        {
            soxFiles.AddRange(Directory.GetFiles(gamePath, "*.sox", SearchOption.AllDirectories));
        }

        foreach (var file in soxFiles)
        {
            var result = ValidateFile(file);
            summary.AddResult(result);
        }

        return summary;
    }
}

/// <summary>
/// Summary of validation across multiple files.
/// </summary>
public class ValidationSummary
{
    private readonly List<ValidationResult> _results = new();

    /// <summary>
    /// Gets the root path that was validated.
    /// </summary>
    public string RootPath { get; }

    /// <summary>
    /// Gets all validation results.
    /// </summary>
    public IReadOnlyList<ValidationResult> Results => _results;

    /// <summary>
    /// Gets the total number of files validated.
    /// </summary>
    public int TotalFiles => _results.Count;

    /// <summary>
    /// Gets the number of files with errors.
    /// </summary>
    public int FilesWithErrors => _results.Count(r => !r.IsValid);

    /// <summary>
    /// Gets the number of files with warnings.
    /// </summary>
    public int FilesWithWarnings => _results.Count(r => r.HasWarnings);

    /// <summary>
    /// Gets the total number of errors across all files.
    /// </summary>
    public int TotalErrors => _results.Sum(r => r.ErrorCount);

    /// <summary>
    /// Gets the total number of warnings across all files.
    /// </summary>
    public int TotalWarnings => _results.Sum(r => r.WarningCount);

    /// <summary>
    /// Gets whether all files passed validation.
    /// </summary>
    public bool AllValid => _results.All(r => r.IsValid);

    public ValidationSummary(string rootPath)
    {
        RootPath = rootPath ?? throw new ArgumentNullException(nameof(rootPath));
    }

    /// <summary>
    /// Adds a validation result.
    /// </summary>
    public void AddResult(ValidationResult result)
    {
        _results.Add(result);
    }

    /// <summary>
    /// Gets results with errors only.
    /// </summary>
    public IEnumerable<ValidationResult> GetErrorResults()
    {
        return _results.Where(r => !r.IsValid);
    }

    /// <summary>
    /// Gets results with warnings only.
    /// </summary>
    public IEnumerable<ValidationResult> GetWarningResults()
    {
        return _results.Where(r => r.HasWarnings && r.IsValid);
    }
}
