namespace KUFEditor.Core.Validation;

/// <summary>
/// Interface for file validators.
/// </summary>
public interface IFileValidator
{
    /// <summary>
    /// Gets the file types this validator can handle.
    /// </summary>
    IEnumerable<string> SupportedExtensions { get; }

    /// <summary>
    /// Checks if this validator can handle the given file.
    /// </summary>
    bool CanValidate(string path);

    /// <summary>
    /// Validates the file at the given path.
    /// </summary>
    ValidationResult Validate(string path);

    /// <summary>
    /// Validates file data from a stream.
    /// </summary>
    ValidationResult Validate(Stream stream, string filePath);
}
