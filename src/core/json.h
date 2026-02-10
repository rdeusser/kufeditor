#pragma once

#include "core/mod_metadata.h"

#include <optional>
#include <string>
#include <vector>

namespace kuf {

struct InstalledModInfo;

std::optional<ModMetadata> parseModJson(const std::string& text);
std::string serializeModJson(const ModMetadata& meta);

std::vector<InstalledModInfo> parseInstalledModsJson(const std::string& text);
std::string serializeInstalledModsJson(const std::vector<InstalledModInfo>& mods);

} // namespace kuf
