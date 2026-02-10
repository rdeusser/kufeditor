#pragma once

#include "core/async_task.h"
#include "core/mod_metadata.h"

#include <string>
#include <variant>
#include <vector>

namespace kuf {

struct ModInfo {
    ModMetadata metadata;
    std::string zipPath;
    size_t fileSize = 0;
};

class ModManager {
public:
    static std::string modsDirectory();
    static bool createMod(const ModMetadata& meta, const std::string& gameDir,
                          const std::vector<std::string>& relativePaths,
                          const std::string& outputZipPath, AsyncTask& task);
    static std::variant<ModInfo, std::string> importMod(const std::string& zipPath);
    static bool applyMod(const ModInfo& mod, const std::string& gameDir, AsyncTask& task);
    static bool removeMod(const ModInfo& mod);
    static std::vector<ModInfo> listMods();
};

} // namespace kuf
