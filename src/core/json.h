#pragma once

#include "core/mod_metadata.h"

#include <optional>
#include <string>

namespace kuf {

std::optional<ModMetadata> parseModJson(const std::string& text);
std::string serializeModJson(const ModMetadata& meta);

} // namespace kuf
