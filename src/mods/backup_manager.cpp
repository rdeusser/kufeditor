#include "mods/backup_manager.h"
#include "core/config.h"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <sstream>

namespace kuf {

namespace fs = std::filesystem;

namespace {

std::string currentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    std::tm tm{};
#ifdef _WIN32
    localtime_s(&tm, &time);
#else
    localtime_r(&time, &tm);
#endif
    std::ostringstream ss;
    ss << std::put_time(&tm, "%Y-%m-%d_%H%M%S");
    return ss.str();
}

std::string formatTimestamp(const std::string& dirName) {
    // Convert "2026-02-10_143022" to "2026-02-10 14:30:22"
    if (dirName.size() < 15) return dirName;
    std::string result = dirName.substr(0, 10) + " ";
    result += dirName.substr(11, 2) + ":";
    result += dirName.substr(13, 2) + ":";
    result += dirName.substr(15, 2);
    return result;
}

void copyFileWithDirs(const fs::path& src, const fs::path& dest) {
    fs::create_directories(dest.parent_path());
    fs::copy_file(src, dest, fs::copy_options::overwrite_existing);
}

std::vector<fs::path> enumerateFiles(const std::string& dir) {
    std::vector<fs::path> files;
    if (!fs::exists(dir)) return files;
    for (const auto& entry : fs::recursive_directory_iterator(dir)) {
        if (entry.is_regular_file()) {
            files.push_back(entry.path());
        }
    }
    return files;
}

} // namespace

std::string BackupManager::backupDirectory() {
    return getConfigDir() + "/backups";
}

bool BackupManager::createBackup(const std::string& gameDir, AsyncTask& task) {
    auto files = enumerateFiles(gameDir);
    if (files.empty()) {
        task.setError("No files found in game directory");
        return false;
    }

    std::string timestamp = currentTimestamp();
    std::string backupDir = backupDirectory() + "/" + timestamp;
    fs::create_directories(backupDir);

    fs::path gameRoot(gameDir);
    size_t totalBytes = 0;

    for (size_t i = 0; i < files.size(); ++i) {
        auto relativePath = fs::relative(files[i], gameRoot);
        auto destPath = fs::path(backupDir) / relativePath;

        task.setProgress(static_cast<float>(i) / static_cast<float>(files.size()),
                         relativePath.string());

        try {
            copyFileWithDirs(files[i], destPath);
            totalBytes += fs::file_size(files[i]);
        } catch (const fs::filesystem_error& e) {
            task.setError(std::string("Failed to copy ") + files[i].string() + ": " + e.what());
            return false;
        }
    }

    // Write backup.json metadata.
    std::ostringstream json;
    json << "{\n";
    json << "  \"gameDirectory\": \"";
    for (char c : gameDir) {
        if (c == '\\') json << "\\\\";
        else if (c == '"') json << "\\\"";
        else json << c;
    }
    json << "\",\n";
    json << "  \"created\": \"" << timestamp << "\",\n";
    json << "  \"fileCount\": " << files.size() << "\n";
    json << "}\n";

    std::ofstream metaFile(backupDir + "/backup.json");
    metaFile << json.str();

    task.setProgress(1.0f, "Backup complete");
    return true;
}

bool BackupManager::restoreBackup(const BackupInfo& backup, const std::string& gameDir, AsyncTask& task) {
    auto files = enumerateFiles(backup.path);

    // Filter out backup.json from the file list.
    std::vector<fs::path> gameFiles;
    for (const auto& f : files) {
        if (f.filename() != "backup.json") {
            gameFiles.push_back(f);
        }
    }

    if (gameFiles.empty()) {
        task.setError("Backup contains no game files");
        return false;
    }

    fs::path backupRoot(backup.path);

    for (size_t i = 0; i < gameFiles.size(); ++i) {
        auto relativePath = fs::relative(gameFiles[i], backupRoot);
        auto destPath = fs::path(gameDir) / relativePath;

        task.setProgress(static_cast<float>(i) / static_cast<float>(gameFiles.size()),
                         relativePath.string());

        try {
            copyFileWithDirs(gameFiles[i], destPath);
        } catch (const fs::filesystem_error& e) {
            task.setError(std::string("Failed to restore ") + relativePath.string() + ": " + e.what());
            return false;
        }
    }

    task.setProgress(1.0f, "Restore complete");
    return true;
}

bool BackupManager::deleteBackup(const BackupInfo& backup) {
    if (backup.path.empty() || !fs::exists(backup.path)) return false;

    std::error_code ec;
    fs::remove_all(backup.path, ec);
    return !ec;
}

std::vector<BackupInfo> BackupManager::listBackups() {
    std::vector<BackupInfo> backups;
    std::string dir = backupDirectory();

    if (!fs::exists(dir)) return backups;

    for (const auto& entry : fs::directory_iterator(dir)) {
        if (!entry.is_directory()) continue;

        BackupInfo info;
        info.path = entry.path().string();
        info.timestamp = formatTimestamp(entry.path().filename().string());

        // Read backup.json if it exists.
        std::string metaPath = info.path + "/backup.json";
        if (fs::exists(metaPath)) {
            std::ifstream file(metaPath);
            std::string content((std::istreambuf_iterator<char>(file)),
                                 std::istreambuf_iterator<char>());

            // Simple extraction of gameDirectory field.
            auto gdPos = content.find("\"gameDirectory\"");
            if (gdPos != std::string::npos) {
                auto valStart = content.find('"', content.find(':', gdPos) + 1);
                if (valStart != std::string::npos) {
                    auto valEnd = content.find('"', valStart + 1);
                    if (valEnd != std::string::npos) {
                        info.gameDirectory = content.substr(valStart + 1, valEnd - valStart - 1);
                    }
                }
            }
        }

        // Count files and total size.
        for (const auto& f : fs::recursive_directory_iterator(entry.path())) {
            if (f.is_regular_file() && f.path().filename() != "backup.json") {
                ++info.fileCount;
                info.totalBytes += f.file_size();
            }
        }

        backups.push_back(std::move(info));
    }

    // Sort by timestamp descending (newest first).
    std::sort(backups.begin(), backups.end(), [](const BackupInfo& a, const BackupInfo& b) {
        return a.timestamp > b.timestamp;
    });

    return backups;
}

std::optional<BackupInfo> BackupManager::latestBackup() {
    auto backups = listBackups();
    if (backups.empty()) return std::nullopt;
    return backups.front();
}

} // namespace kuf
