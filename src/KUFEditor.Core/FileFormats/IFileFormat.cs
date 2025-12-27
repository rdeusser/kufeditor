namespace KUFEditor.Core.FileFormats;

/// <summary>
/// Interface for all file format handlers.
/// </summary>
public interface IFileFormat
{
    string Extension { get; }
    string Description { get; }

    bool CanRead(string path);
    bool CanWrite(string path);

    object Read(string path);
    void Write(string path, object data);
}