using System.Collections.Generic;

namespace KUFEditor.Core.Mods;

/// <summary>
/// Interface for patching SOX files.
/// </summary>
public interface ISoxPatcher
{
    /// <summary>
    /// Checks if this patcher can handle the specified file.
    /// </summary>
    bool CanHandle(string fileName);

    /// <summary>
    /// Applies a modification patch to a record.
    /// </summary>
    void Modify(string filePath, string recordName, Dictionary<string, object> fields);

    /// <summary>
    /// Adds a new record to the file.
    /// </summary>
    void Add(string filePath, Dictionary<string, object> data);

    /// <summary>
    /// Deletes a record from the file.
    /// </summary>
    void Delete(string filePath, string recordName);
}
