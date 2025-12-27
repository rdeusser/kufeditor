namespace KUFEditor.Assets;

/// <summary>
/// Represents a SOX file used in Kingdom Under Fire.
/// </summary>
public class SoxFile
{
    public string FileName { get; set; } = string.Empty;
    public byte[] Data { get; set; } = Array.Empty<byte>();
    public SoxFileType Type { get; set; }

    public static SoxFile Load(string path)
    {
        var sox = new SoxFile
        {
            FileName = Path.GetFileName(path),
            Data = File.ReadAllBytes(path)
        };

        // determine type based on header or extension
        return sox;
    }

    public void Save(string path)
    {
        File.WriteAllBytes(path, Data);
    }
}

public enum SoxFileType
{
    Unknown,
    Model,
    Texture,
    Animation,
    Sound
}