#pragma once

#include <optional>
#include <string>

namespace kuf {

class FileDialog {
public:
    static std::optional<std::string> openFile(const char* filter, const char* initialDir = nullptr);
    static std::optional<std::string> saveFile(const char* filter, const char* defaultName = nullptr);
    static std::optional<std::string> openFolder();
};

} // namespace kuf
