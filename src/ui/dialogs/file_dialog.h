#pragma once

#include <optional>
#include <string>

namespace kuf {

class FileDialog {
public:
    static std::optional<std::string> openFile(const char* filter);
    static std::optional<std::string> saveFile(const char* filter, const char* defaultName = nullptr);
};

} // namespace kuf
