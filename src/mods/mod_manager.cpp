#include "mods/mod_manager.h"
#include "core/config.h"
#include "core/json.h"
#include "core/zip_archive.h"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <sstream>

namespace kuf {

namespace fs = std::filesystem;

std::string ModManager::modsDirectory() {
    return getConfigDir() + "/mods";
}

bool ModManager::createMod(const ModMetadata& meta, const std::string& gameDir,
                           const std::vector<std::string>& relativePaths,
                           const std::string& outputZipPath, AsyncTask& task) {
    ZipWriter writer;
    if (!writer.create(outputZipPath)) {
        task.setError("Failed to create zip file");
        return false;
    }

    // Add mod.json.
    std::string jsonStr = serializeModJson(meta);
    if (!writer.addMemory("mod.json", jsonStr.data(), jsonStr.size())) {
        task.setError("Failed to write mod.json to archive");
        return false;
    }

    // Add game files.
    for (size_t i = 0; i < relativePaths.size(); ++i) {
        std::string diskPath = gameDir + "/" + relativePaths[i];

        task.setProgress(static_cast<float>(i) / static_cast<float>(relativePaths.size()),
                         relativePaths[i]);

        if (!writer.addFile(diskPath, relativePaths[i])) {
            task.setError("Failed to add file: " + relativePaths[i]);
            return false;
        }
    }

    if (!writer.finalize()) {
        task.setError("Failed to finalize zip archive");
        return false;
    }

    task.setProgress(1.0f, "Mod created");
    return true;
}

std::variant<ModInfo, std::string> ModManager::importMod(const std::string& zipPath) {
    ZipReader reader;
    if (!reader.open(zipPath)) {
        return std::string("Failed to open zip file");
    }

    // Read and validate mod.json.
    auto jsonData = reader.readEntry("mod.json");
    if (!jsonData) {
        return std::string("Archive does not contain mod.json");
    }

    std::string jsonStr(reinterpret_cast<const char*>(jsonData->data()), jsonData->size());
    auto meta = parseModJson(jsonStr);
    if (!meta) {
        return std::string("Invalid or incomplete mod.json (requires name, version, game, files)");
    }

    // Copy to mods directory.
    fs::create_directories(modsDirectory());
    std::string destPath = modsDirectory() + "/" + fs::path(zipPath).filename().string();

    // Avoid overwriting: append number if needed.
    if (fs::exists(destPath) && fs::canonical(zipPath) != fs::canonical(destPath)) {
        std::string stem = fs::path(zipPath).stem().string();
        std::string ext = fs::path(zipPath).extension().string();
        int n = 1;
        while (fs::exists(destPath)) {
            destPath = modsDirectory() + "/" + stem + "_" + std::to_string(n) + ext;
            ++n;
        }
    }

    // Only copy if source and destination differ.
    if (!fs::exists(destPath) || fs::canonical(zipPath) != fs::canonical(destPath)) {
        std::error_code ec;
        fs::copy_file(zipPath, destPath, fs::copy_options::overwrite_existing, ec);
        if (ec) {
            return std::string("Failed to copy mod to library: " + ec.message());
        }
    }

    ModInfo info;
    info.metadata = *meta;
    info.zipPath = destPath;
    info.fileSize = fs::file_size(destPath);
    return info;
}

bool ModManager::applyMod(const ModInfo& mod, const std::string& gameDir, AsyncTask& task) {
    ZipReader reader;
    if (!reader.open(mod.zipPath)) {
        task.setError("Failed to open mod archive");
        return false;
    }

    auto allEntries = reader.entries();

    // Filter out mod.json.
    std::vector<std::string> gameEntries;
    for (const auto& entry : allEntries) {
        if (entry != "mod.json") {
            gameEntries.push_back(entry);
        }
    }

    for (size_t i = 0; i < gameEntries.size(); ++i) {
        task.setProgress(static_cast<float>(i) / static_cast<float>(gameEntries.size()),
                         gameEntries[i]);

        std::string destPath = gameDir + "/" + gameEntries[i];
        if (!reader.extractEntry(gameEntries[i], destPath)) {
            task.setError("Failed to extract: " + gameEntries[i]);
            return false;
        }
    }

    task.setProgress(1.0f, "Mod applied");
    return true;
}

bool ModManager::removeMod(const ModInfo& mod) {
    if (mod.zipPath.empty() || !fs::exists(mod.zipPath)) return false;
    std::error_code ec;
    fs::remove(mod.zipPath, ec);
    return !ec;
}

std::vector<ModInfo> ModManager::listMods() {
    std::vector<ModInfo> mods;
    std::string dir = modsDirectory();

    if (!fs::exists(dir)) return mods;

    for (const auto& entry : fs::directory_iterator(dir)) {
        if (!entry.is_regular_file()) continue;
        if (entry.path().extension() != ".zip") continue;

        ZipReader reader;
        if (!reader.open(entry.path().string())) continue;

        auto jsonData = reader.readEntry("mod.json");
        if (!jsonData) continue;

        std::string jsonStr(reinterpret_cast<const char*>(jsonData->data()), jsonData->size());
        auto meta = parseModJson(jsonStr);
        if (!meta) continue;

        ModInfo info;
        info.metadata = *meta;
        info.zipPath = entry.path().string();
        info.fileSize = entry.file_size();
        mods.push_back(std::move(info));
    }

    // Sort by name.
    std::sort(mods.begin(), mods.end(), [](const ModInfo& a, const ModInfo& b) {
        return a.metadata.name < b.metadata.name;
    });

    return mods;
}

std::vector<InstalledModInfo> ModManager::listInstalledMods() {
    std::string path = modsDirectory() + "/installed.json";
    if (!fs::exists(path)) return {};

    std::ifstream file(path);
    if (!file) return {};

    std::string content((std::istreambuf_iterator<char>(file)),
                        std::istreambuf_iterator<char>());
    return parseInstalledModsJson(content);
}

bool ModManager::markInstalled(const ModInfo& mod) {
    auto mods = listInstalledMods();

    // Replace existing entry with the same name.
    bool replaced = false;
    for (auto& m : mods) {
        if (m.name == mod.metadata.name) {
            m.version = mod.metadata.version;
            m.author = mod.metadata.author;
            m.game = mod.metadata.game;
            m.zipPath = mod.zipPath;

            auto now = std::chrono::system_clock::now();
            auto time = std::chrono::system_clock::to_time_t(now);
            std::tm tm{};
#ifdef _WIN32
            gmtime_s(&tm, &time);
#else
            gmtime_r(&time, &tm);
#endif
            std::ostringstream ss;
            ss << std::put_time(&tm, "%Y-%m-%dT%H:%M:%SZ");
            m.installedAt = ss.str();

            replaced = true;
            break;
        }
    }

    if (!replaced) {
        InstalledModInfo info;
        info.name = mod.metadata.name;
        info.version = mod.metadata.version;
        info.author = mod.metadata.author;
        info.game = mod.metadata.game;
        info.zipPath = mod.zipPath;

        auto now = std::chrono::system_clock::now();
        auto time = std::chrono::system_clock::to_time_t(now);
        std::tm tm{};
#ifdef _WIN32
        gmtime_s(&tm, &time);
#else
        gmtime_r(&time, &tm);
#endif
        std::ostringstream ss;
        ss << std::put_time(&tm, "%Y-%m-%dT%H:%M:%SZ");
        info.installedAt = ss.str();

        mods.push_back(std::move(info));
    }

    fs::create_directories(modsDirectory());
    std::string path = modsDirectory() + "/installed.json";
    std::ofstream file(path);
    if (!file) return false;

    file << serializeInstalledModsJson(mods);
    return file.good();
}

bool ModManager::markUninstalled(const std::string& name) {
    auto mods = listInstalledMods();

    auto it = std::remove_if(mods.begin(), mods.end(),
                             [&name](const InstalledModInfo& m) { return m.name == name; });
    if (it == mods.end()) return false;
    mods.erase(it, mods.end());

    std::string path = modsDirectory() + "/installed.json";
    std::ofstream file(path);
    if (!file) return false;

    file << serializeInstalledModsJson(mods);
    return file.good();
}

} // namespace kuf
