namespace KUFEditor.Core;

/// <summary>
/// Manages backup operations for game files.
/// </summary>
public class BackupManager(string backupRoot)
{
    private readonly string _backupRoot = backupRoot;

    /// <summary>
    /// Creates a backup of the specified file.
    /// </summary>
    public string BackupFile(string sourceFilePath, string gameName)
    {
        if (!File.Exists(sourceFilePath))
        {
            throw new FileNotFoundException($"Source file not found: {sourceFilePath}");
        }

        var gameBackupDir = Settings.GetGameBackupDirectory(_backupRoot, gameName);

        // create timestamped backup directory
        var timestamp = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
        var backupDir = Path.Combine(gameBackupDir, timestamp);

        if (!Directory.Exists(backupDir))
        {
            Directory.CreateDirectory(backupDir);
        }

        var fileName = Path.GetFileName(sourceFilePath);
        var destinationPath = Path.Combine(backupDir, fileName);

        File.Copy(sourceFilePath, destinationPath, overwrite: true);

        return destinationPath;
    }

    /// <summary>
    /// Backs up all SOX files from the specified game directory.
    /// </summary>
    public List<string> BackupAllSoxFiles(string gameDirectory, string gameName)
    {
        if (!Directory.Exists(gameDirectory))
        {
            throw new DirectoryNotFoundException($"Game directory not found: {gameDirectory}");
        }

        var soxDir = Path.Combine(gameDirectory, "Data", "SOX");
        if (!Directory.Exists(soxDir))
        {
            throw new DirectoryNotFoundException($"SOX directory not found: {soxDir}");
        }

        var gameBackupDir = Settings.GetGameBackupDirectory(_backupRoot, gameName);

        // create timestamped backup directory
        var timestamp = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
        var backupDir = Path.Combine(gameBackupDir, timestamp);

        if (!Directory.Exists(backupDir))
        {
            Directory.CreateDirectory(backupDir);
        }

        var backedUpFiles = new List<string>();
        var soxFiles = Directory.GetFiles(soxDir, "*.sox", SearchOption.TopDirectoryOnly);

        foreach (var soxFile in soxFiles)
        {
            var fileName = Path.GetFileName(soxFile);
            var destinationPath = Path.Combine(backupDir, fileName);

            File.Copy(soxFile, destinationPath, overwrite: true);
            backedUpFiles.Add(destinationPath);
        }

        return backedUpFiles;
    }

    /// <summary>
    /// Gets all backup directories for a specific game, sorted by date (newest first).
    /// </summary>
    public List<string> GetBackups(string gameName)
    {
        var gameBackupDir = Settings.GetGameBackupDirectory(_backupRoot, gameName);

        if (!Directory.Exists(gameBackupDir))
        {
            return new List<string>();
        }

        var directories = Directory.GetDirectories(gameBackupDir);
        return directories
            .OrderByDescending(d => Directory.GetCreationTime(d))
            .ToList();
    }

    /// <summary>
    /// Restores a file from a backup.
    /// </summary>
    public void RestoreFile(string backupFilePath, string destinationPath)
    {
        if (!File.Exists(backupFilePath))
        {
            throw new FileNotFoundException($"Backup file not found: {backupFilePath}");
        }

        var destDir = Path.GetDirectoryName(destinationPath);
        if (destDir != null && !Directory.Exists(destDir))
        {
            Directory.CreateDirectory(destDir);
        }

        File.Copy(backupFilePath, destinationPath, overwrite: true);
    }

    /// <summary>
    /// Deletes old backups, keeping only the specified number of most recent backups.
    /// </summary>
    public void CleanOldBackups(string gameName, int keepCount = 10)
    {
        var backups = GetBackups(gameName);

        if (backups.Count <= keepCount)
        {
            return;
        }

        // delete older backups
        for (int i = keepCount; i < backups.Count; i++)
        {
            try
            {
                Directory.Delete(backups[i], recursive: true);
            }
            catch
            {
                // ignore errors when deleting old backups
            }
        }
    }
}