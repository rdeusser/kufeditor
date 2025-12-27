using System.Text.Json;

namespace KUFEditor.Core;

/// <summary>
/// Application settings and configuration.
/// </summary>
public class Settings
{
    public string? CrusadersPath { get; set; }
    public string? HeroesPath { get; set; }
    public string? BackupDirectory { get; set; }

    /// <summary>
    /// Gets the default backup directory path based on the current platform.
    /// </summary>
    public static string GetDefaultBackupDirectory()
    {
        if (OperatingSystem.IsWindows())
        {
            var docs = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
            return Path.Combine(docs, "KUFBackup");
        }

        if (OperatingSystem.IsMacOS())
        {
            var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            return Path.Combine(home, "Library", "Application Support", "KUFBackup");
        }

        // linux/unix
        var xdgConfig = Environment.GetEnvironmentVariable("XDG_CONFIG_HOME");
        if (!string.IsNullOrEmpty(xdgConfig) && Path.IsPathFullyQualified(xdgConfig))
        {
            return Path.Combine(xdgConfig, "KUFBackup");
        }

        var homeDir = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        return Path.Combine(homeDir, ".config", "KUFBackup");
    }

    /// <summary>
    /// Gets the backup subdirectory for a specific game.
    /// </summary>
    public static string GetGameBackupDirectory(string backupRoot, string gameName)
    {
        return Path.Combine(backupRoot, gameName);
    }

    /// <summary>
    /// Saves settings to the specified file path.
    /// </summary>
    public void Save(string filePath)
    {
        var directory = Path.GetDirectoryName(filePath);
        if (directory != null && !Directory.Exists(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var json = JsonSerializer.Serialize(this, new JsonSerializerOptions
        {
            WriteIndented = true
        });

        File.WriteAllText(filePath, json);
    }

    /// <summary>
    /// Loads settings from the specified file path.
    /// </summary>
    public static Settings Load(string filePath)
    {
        if (!File.Exists(filePath))
        {
            var settings = new Settings
            {
                BackupDirectory = GetDefaultBackupDirectory()
            };
            return settings;
        }

        var json = File.ReadAllText(filePath);
        var loaded = JsonSerializer.Deserialize<Settings>(json);

        if (loaded == null)
        {
            return new Settings
            {
                BackupDirectory = GetDefaultBackupDirectory()
            };
        }

        // ensure backup directory is set
        if (string.IsNullOrEmpty(loaded.BackupDirectory))
        {
            loaded.BackupDirectory = GetDefaultBackupDirectory();
        }

        return loaded;
    }

    /// <summary>
    /// Gets the default settings file path.
    /// </summary>
    public static string GetDefaultSettingsPath()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "KUFEditor", "settings.json");
    }
}