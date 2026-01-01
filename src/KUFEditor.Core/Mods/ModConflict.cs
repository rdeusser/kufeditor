namespace KUFEditor.Core.Mods;

/// <summary>
/// Represents a conflict where two mods modify the same field.
/// </summary>
public class ModConflict
{
    public string File { get; set; } = string.Empty;
    public string Record { get; set; } = string.Empty;
    public string Field { get; set; } = string.Empty;
    public string FirstModId { get; set; } = string.Empty;
    public object? FirstValue { get; set; }
    public string SecondModId { get; set; } = string.Empty;
    public object? SecondValue { get; set; }

    public override string ToString()
    {
        return $"{File} / {Record} / {Field}: '{FirstModId}' set to {FirstValue}, '{SecondModId}' overwrote with {SecondValue}";
    }
}
