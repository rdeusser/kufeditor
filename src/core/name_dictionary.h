#pragma once

#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>

namespace kuf {

struct SpecialNameEntry {
    std::vector<std::byte> keyBytes;
    std::string displayName;
};

class NameDictionary {
public:
    bool load(const std::string& soxDir);

    const char* troopInfoName(uint32_t index) const;
    const char* charInfoName(uint8_t jobType) const;
    const std::vector<SpecialNameEntry>& specialNames() const { return specialNames_; }

    std::string translate(const std::string& korean) const;

    bool loaded() const { return loaded_; }

private:
    bool loadIndexedTextSox(const std::string& path, std::vector<std::string>& entries);
    bool loadSpecialNamesSox(const std::string& soxPath, const std::string& localizedPath);
    std::vector<std::byte> readSoxFile(const std::string& path);

    std::vector<std::string> troopInfoNames_;
    std::vector<std::string> charInfoNames_;
    std::vector<SpecialNameEntry> specialNames_;
    std::unordered_map<std::string, std::string> koreanToEnglish_;
    bool loaded_ = false;
};

std::string findGameDirectory(const std::string& stgFilePath);

} // namespace kuf
