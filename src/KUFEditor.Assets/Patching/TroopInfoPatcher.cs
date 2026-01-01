using System;
using System.Collections.Generic;
using System.Reflection;
using KUFEditor.Assets.TroopInfo;
using KUFEditor.Core.Mods;

namespace KUFEditor.Assets.Patching;

/// <summary>
/// Patches TroopInfo.sox files.
/// </summary>
public class TroopInfoPatcher : ISoxPatcher
{
    private readonly TroopInfoSoxFile _parser = new();

    public bool CanHandle(string fileName)
    {
        return fileName.Equals("TroopInfo.sox", StringComparison.OrdinalIgnoreCase);
    }

    public void Modify(string filePath, string recordName, Dictionary<string, object> fields)
    {
        var sox = (TroopInfoSox)_parser.Read(filePath);
        var index = FindTroopIndex(recordName);

        if (index < 0)
            throw new InvalidOperationException($"Troop '{recordName}' not found");

        var troop = sox.TroopInfos[index];
        ApplyFields(troop, fields);

        _parser.Write(filePath, sox);
    }

    public void Add(string filePath, Dictionary<string, object> data)
    {
        // TroopInfo.sox has a fixed array of 43 troops, so adding is not supported.
        throw new NotSupportedException("TroopInfo.sox has a fixed structure. Cannot add new troops.");
    }

    public void Delete(string filePath, string recordName)
    {
        // TroopInfo.sox has a fixed array of 43 troops, so deletion is not supported.
        throw new NotSupportedException("TroopInfo.sox has a fixed structure. Cannot delete troops.");
    }

    private static int FindTroopIndex(string name)
    {
        for (int i = 0; i < TroopNames.Names.Length; i++)
        {
            if (TroopNames.Names[i].Equals(name, StringComparison.OrdinalIgnoreCase))
                return i;
        }
        return -1;
    }

    private static void ApplyFields(TroopInfo.TroopInfo troop, Dictionary<string, object> fields)
    {
        var type = typeof(TroopInfo.TroopInfo);

        foreach (var (fieldName, value) in fields)
        {
            var prop = type.GetProperty(fieldName, BindingFlags.Public | BindingFlags.Instance | BindingFlags.IgnoreCase);
            if (prop == null)
                throw new InvalidOperationException($"Unknown field '{fieldName}' on TroopInfo");

            var converted = ConvertValue(value, prop.PropertyType);
            prop.SetValue(troop, converted);
        }
    }

    private static object? ConvertValue(object value, Type targetType)
    {
        if (value == null) return null;

        // Handle JsonElement from deserialization.
        if (value is System.Text.Json.JsonElement je)
        {
            return targetType switch
            {
                Type t when t == typeof(int) => je.GetInt32(),
                Type t when t == typeof(float) => je.GetSingle(),
                Type t when t == typeof(double) => je.GetDouble(),
                Type t when t == typeof(string) => je.GetString(),
                Type t when t == typeof(bool) => je.GetBoolean(),
                _ => throw new InvalidOperationException($"Cannot convert JsonElement to {targetType.Name}")
            };
        }

        return Convert.ChangeType(value, targetType);
    }
}
