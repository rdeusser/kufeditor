namespace KUFEditor.Core;

/// <summary>
/// Manages backup operations for game files including pristine backups and named snapshots.
/// </summary>
public class BackupManager
{
    private readonly string _backupRoot;

    public BackupManager(string backupRoot)
    {
        _backupRoot = backupRoot;
    }

    /// <summary>
    /// Gets the backup root directory.
    /// </summary>
    public string BackupRoot => _backupRoot;

    // ========== Pristine Backups ==========

    /// <summary>
    /// Captures pristine (original) copies of all game files. Should be called once when game path is first set.
    /// </summary>
    public void CapturePristine(string gamePath, string gameName)
    {
        if (!Directory.Exists(gamePath))
            throw new DirectoryNotFoundException($"Game directory not found: {gamePath}");

        var pristineDir = GetPristineDirectory(gameName);
        if (!Directory.Exists(pristineDir))
            Directory.CreateDirectory(pristineDir);

        // capture SOX files
        var soxPath = Path.Combine(gamePath, "Data", "SOX");
        if (!Directory.Exists(soxPath))
            soxPath = Path.Combine(gamePath, "Data", "Sox");

        if (Directory.Exists(soxPath))
        {
            CaptureDirectoryPristine(soxPath, Path.Combine(pristineDir, "SOX"));
        }

        // capture Mission files
        var missionPath = Path.Combine(gamePath, "Data", "Mission");
        if (Directory.Exists(missionPath))
        {
            CaptureDirectoryPristine(missionPath, Path.Combine(pristineDir, "Mission"));
        }
    }

    private static void CaptureDirectoryPristine(string sourceDir, string destDir)
    {
        if (!Directory.Exists(destDir))
            Directory.CreateDirectory(destDir);

        foreach (var file in Directory.GetFiles(sourceDir))
        {
            var destFile = Path.Combine(destDir, Path.GetFileName(file));
            if (!File.Exists(destFile))
            {
                File.Copy(file, destFile);
            }
        }

        foreach (var subDir in Directory.GetDirectories(sourceDir))
        {
            var destSubDir = Path.Combine(destDir, Path.GetFileName(subDir));
            CaptureDirectoryPristine(subDir, destSubDir);
        }
    }

    /// <summary>
    /// Checks if a pristine backup exists for a specific file.
    /// </summary>
    public bool HasPristineBackup(string gameName, string fileName)
    {
        var pristineDir = GetPristineDirectory(gameName);
        return FindPristineFile(pristineDir, fileName) != null;
    }

    /// <summary>
    /// Gets the path to the pristine backup for a file, or null if not found.
    /// </summary>
    public string? GetPristinePath(string gameName, string fileName)
    {
        var pristineDir = GetPristineDirectory(gameName);
        return FindPristineFile(pristineDir, fileName);
    }

    private static string? FindPristineFile(string dir, string fileName)
    {
        if (!Directory.Exists(dir)) return null;

        var directPath = Path.Combine(dir, fileName);
        if (File.Exists(directPath)) return directPath;

        foreach (var subDir in Directory.GetDirectories(dir))
        {
            var found = FindPristineFile(subDir, fileName);
            if (found != null) return found;
        }

        return null;
    }

    /// <summary>
    /// Restores a file from its pristine backup.
    /// </summary>
    public void RestoreFromPristine(string gameName, string fileName, string destinationPath)
    {
        var pristinePath = GetPristinePath(gameName, fileName)
            ?? throw new FileNotFoundException($"No pristine backup found for: {fileName}");

        var destDir = Path.GetDirectoryName(destinationPath);
        if (destDir != null && !Directory.Exists(destDir))
            Directory.CreateDirectory(destDir);

        File.Copy(pristinePath, destinationPath, overwrite: true);
    }

    private string GetPristineDirectory(string gameName)
    {
        return Path.Combine(_backupRoot, gameName, "pristine");
    }

    // ========== Named Snapshots ==========

    /// <summary>
    /// Creates a named snapshot of a file.
    /// </summary>
    public string CreateSnapshot(string gameName, string sourceFilePath, string snapshotName)
    {
        if (!File.Exists(sourceFilePath))
            throw new FileNotFoundException($"Source file not found: {sourceFilePath}");

        var fileName = Path.GetFileName(sourceFilePath);
        var snapshotDir = GetSnapshotDirectory(gameName, fileName, snapshotName);

        if (!Directory.Exists(snapshotDir))
            Directory.CreateDirectory(snapshotDir);

        var destPath = Path.Combine(snapshotDir, fileName);
        File.Copy(sourceFilePath, destPath, overwrite: true);

        return destPath;
    }

    /// <summary>
    /// Gets all snapshots for a specific file.
    /// </summary>
    public List<SnapshotInfo> GetSnapshots(string gameName, string fileName)
    {
        var result = new List<SnapshotInfo>();
        var fileSnapshotsDir = Path.Combine(_backupRoot, gameName, "snapshots", fileName);

        if (!Directory.Exists(fileSnapshotsDir))
            return result;

        foreach (var snapshotDir in Directory.GetDirectories(fileSnapshotsDir))
        {
            var snapshotName = Path.GetFileName(snapshotDir);
            var snapshotFile = Path.Combine(snapshotDir, fileName);

            if (File.Exists(snapshotFile))
            {
                result.Add(new SnapshotInfo
                {
                    Name = snapshotName,
                    Path = snapshotFile,
                    Created = File.GetCreationTime(snapshotFile)
                });
            }
        }

        return result.OrderByDescending(s => s.Created).ToList();
    }

    /// <summary>
    /// Restores a file from a snapshot.
    /// </summary>
    public void RestoreSnapshot(string snapshotFilePath, string destinationPath)
    {
        if (!File.Exists(snapshotFilePath))
            throw new FileNotFoundException($"Snapshot file not found: {snapshotFilePath}");

        var destDir = Path.GetDirectoryName(destinationPath);
        if (destDir != null && !Directory.Exists(destDir))
            Directory.CreateDirectory(destDir);

        File.Copy(snapshotFilePath, destinationPath, overwrite: true);
    }

    /// <summary>
    /// Deletes a named snapshot.
    /// </summary>
    public void DeleteSnapshot(string gameName, string fileName, string snapshotName)
    {
        var snapshotDir = GetSnapshotDirectory(gameName, fileName, snapshotName);

        if (Directory.Exists(snapshotDir))
        {
            Directory.Delete(snapshotDir, recursive: true);
        }
    }

    private string GetSnapshotDirectory(string gameName, string fileName, string snapshotName)
    {
        return Path.Combine(_backupRoot, gameName, "snapshots", fileName, snapshotName);
    }

    // ========== Legacy Methods (for compatibility) ==========

    /// <summary>
    /// Creates a backup of the specified file (legacy timestamped backup).
    /// </summary>
    public string BackupFile(string sourceFilePath, string gameName)
    {
        if (!File.Exists(sourceFilePath))
            throw new FileNotFoundException($"Source file not found: {sourceFilePath}");

        var gameBackupDir = Settings.GetGameBackupDirectory(_backupRoot, gameName);
        var timestamp = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
        var backupDir = Path.Combine(gameBackupDir, timestamp);

        if (!Directory.Exists(backupDir))
            Directory.CreateDirectory(backupDir);

        var fileName = Path.GetFileName(sourceFilePath);
        var destinationPath = Path.Combine(backupDir, fileName);

        File.Copy(sourceFilePath, destinationPath, overwrite: true);

        return destinationPath;
    }

    /// <summary>
    /// Backs up all SOX files from the specified game directory (legacy).
    /// </summary>
    public List<string> BackupAllSoxFiles(string gameDirectory, string gameName)
    {
        if (!Directory.Exists(gameDirectory))
            throw new DirectoryNotFoundException($"Game directory not found: {gameDirectory}");

        var soxDir = Path.Combine(gameDirectory, "Data", "SOX");
        if (!Directory.Exists(soxDir))
            throw new DirectoryNotFoundException($"SOX directory not found: {soxDir}");

        var gameBackupDir = Settings.GetGameBackupDirectory(_backupRoot, gameName);
        var timestamp = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
        var backupDir = Path.Combine(gameBackupDir, timestamp);

        if (!Directory.Exists(backupDir))
            Directory.CreateDirectory(backupDir);

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
            return new List<string>();

        var directories = Directory.GetDirectories(gameBackupDir);
        return directories
            .OrderByDescending(d => Directory.GetCreationTime(d))
            .ToList();
    }

    /// <summary>
    /// Restores a file from a backup (legacy).
    /// </summary>
    public void RestoreFile(string backupFilePath, string destinationPath)
    {
        if (!File.Exists(backupFilePath))
            throw new FileNotFoundException($"Backup file not found: {backupFilePath}");

        var destDir = Path.GetDirectoryName(destinationPath);
        if (destDir != null && !Directory.Exists(destDir))
            Directory.CreateDirectory(destDir);

        File.Copy(backupFilePath, destinationPath, overwrite: true);
    }

    /// <summary>
    /// Deletes old backups, keeping only the specified number of most recent backups.
    /// </summary>
    public void CleanOldBackups(string gameName, int keepCount = 10)
    {
        var backups = GetBackups(gameName);

        if (backups.Count <= keepCount)
            return;

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

/// <summary>
/// Information about a named snapshot.
/// </summary>
public class SnapshotInfo
{
    public string Name { get; set; } = string.Empty;
    public string Path { get; set; } = string.Empty;
    public DateTime Created { get; set; }
}
