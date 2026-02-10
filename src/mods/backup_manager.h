#pragma once

#include "core/async_task.h"

#include <optional>
#include <string>
#include <vector>

namespace kuf {

struct BackupInfo {
    std::string path;
    std::string timestamp;
    std::string gameDirectory;
    size_t fileCount = 0;
    size_t totalBytes = 0;
};

class BackupManager {
public:
    static std::string backupDirectory();
    static bool createBackup(const std::string& gameDir, AsyncTask& task);
    static bool restoreBackup(const BackupInfo& backup, const std::string& gameDir, AsyncTask& task);
    static bool deleteBackup(const BackupInfo& backup);
    static std::vector<BackupInfo> listBackups();
    static std::optional<BackupInfo> latestBackup();
};

} // namespace kuf
