namespace KUFEditor.Assets.TroopInfo;

/// <summary>
/// Represents the complete TroopInfo.sox file structure.
/// </summary>
public class TroopInfoSox
{
    public const int VALID_VERSION = 100;
    public const int TROOP_COUNT = 43;
    public const int PADDING_SIZE = 64;

    public int Version { get; set; }
    public int Count { get; set; }

    // Fixed array of 43 troops
    public TroopInfo[] TroopInfos { get; set; } = new TroopInfo[TROOP_COUNT];

    // 64-byte padding at the end of the file
    public byte[] TheEnd { get; set; } = new byte[PADDING_SIZE];

    public TroopInfoSox()
    {
        // Initialize all troop info objects
        for (int i = 0; i < TROOP_COUNT; i++)
        {
            TroopInfos[i] = new TroopInfo();
        }
    }

    public bool IsValid()
    {
        return Version == VALID_VERSION && Count == TROOP_COUNT;
    }
}