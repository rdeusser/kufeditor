#pragma once

#include <string>
#include <vector>

namespace kuf {

struct ModMetadata {
    std::string name;
    std::string version;
    std::string author;
    std::string description;
    std::string game; // "crusaders" or "heroes"
    std::string created; // ISO 8601
    std::vector<std::string> files;
};

} // namespace kuf
