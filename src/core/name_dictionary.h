#pragma once

#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>

namespace kuf {

class NameDictionary {
public:
    bool load(const std::string& soxDir);

    const char* troopName(uint8_t animId) const;
    const char* troopNameByIndex(uint32_t troopInfoIndex) const;
    std::string translate(const std::string& korean) const;

    bool loaded() const { return loaded_; }

private:
    bool loadIndexedTextSox(const std::string& path, std::vector<std::string>& entries);
    bool loadNonIndexedTextSox(const std::string& path, std::vector<std::string>& entries);
    std::vector<std::byte> readSoxFile(const std::string& path);

    std::vector<std::string> troopNames_;
    std::unordered_map<std::string, std::string> koreanToEnglish_;
    bool loaded_ = false;
};

std::string findGameDirectory(const std::string& stgFilePath);

} // namespace kuf
