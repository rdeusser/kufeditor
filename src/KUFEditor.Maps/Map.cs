namespace KUFEditor.Maps;

/// <summary>
/// Represents a Kingdom Under Fire map.
/// </summary>
public class Map
{
    public string Name { get; set; } = string.Empty;
    public int Width { get; set; }
    public int Height { get; set; }
    public MapTile[,]? Tiles { get; set; }
    public List<MapObject> Objects { get; set; } = new();
}

public class MapTile
{
    public int X { get; set; }
    public int Y { get; set; }
    public int TerrainType { get; set; }
    public float Elevation { get; set; }
}

public class MapObject
{
    public string Id { get; set; } = string.Empty;
    public float X { get; set; }
    public float Y { get; set; }
    public float Z { get; set; }
    public float Rotation { get; set; }
    public string Type { get; set; } = string.Empty;
}