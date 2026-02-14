#include "core/name_dictionary.h"

#include "core/text_encoding.h"
#include "formats/sox_encoding.h"

#include <cstring>
#include <filesystem>
#include <fstream>

namespace kuf {

namespace fs = std::filesystem;

namespace {

uint16_t readU16LE(const std::byte* p) {
    return static_cast<uint16_t>(static_cast<uint8_t>(p[0])) |
           (static_cast<uint16_t>(static_cast<uint8_t>(p[1])) << 8);
}

uint32_t readU32LE(const std::byte* p) {
    return static_cast<uint32_t>(static_cast<uint8_t>(p[0])) |
           (static_cast<uint32_t>(static_cast<uint8_t>(p[1])) << 8) |
           (static_cast<uint32_t>(static_cast<uint8_t>(p[2])) << 16) |
           (static_cast<uint32_t>(static_cast<uint8_t>(p[3])) << 24);
}

std::string stripDelimiters(const std::string& s) {
    std::string result = s;
    while (result.size() >= 2 && result.substr(0, 2) == "--") {
        result = result.substr(2);
    }
    while (result.size() >= 2 && result.substr(result.size() - 2) == "--") {
        result = result.substr(0, result.size() - 2);
    }
    return result;
}

std::string stripTrailingDigits(const std::string& s) {
    size_t end = s.size();
    while (end > 0 && s[end - 1] >= '0' && s[end - 1] <= '9') {
        --end;
    }
    if (end == 0 || end == s.size()) return {};
    return s.substr(0, end);
}

bool isThendMarker(const std::byte* data, size_t remaining) {
    if (remaining < 5) return false;
    return std::strncmp(reinterpret_cast<const char*>(data), "THEND", 5) == 0;
}

} // namespace

std::vector<std::byte> NameDictionary::readSoxFile(const std::string& path) {
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file) return {};

    auto size = file.tellg();
    if (size <= 0) return {};
    file.seekg(0);

    std::vector<std::byte> data(static_cast<size_t>(size));
    file.read(reinterpret_cast<char*>(data.data()), size);
    if (!file) return {};

    if (isSoxEncoded(data)) {
        auto decoded = soxDecode(data);
        if (decoded) return std::move(*decoded);
        return {};
    }
    return data;
}

bool NameDictionary::loadIndexedTextSox(const std::string& path, std::vector<std::string>& entries) {
    auto data = readSoxFile(path);
    if (data.size() < 8) return false;

    uint32_t version = readU32LE(data.data());
    uint32_t count = readU32LE(data.data() + 4);
    if (version != 100 || count == 0) return false;

    size_t offset = 8;
    for (uint32_t i = 0; i < count; ++i) {
        // Indexed format: 4-byte index + 2-byte length + text.
        if (offset + 6 > data.size()) break;

        uint32_t index = readU32LE(data.data() + offset);
        uint16_t len = readU16LE(data.data() + offset + 4);
        offset += 6;

        if (offset + len > data.size()) break;

        std::string text(reinterpret_cast<const char*>(data.data() + offset), len);
        offset += len;

        if (index >= entries.size()) {
            entries.resize(index + 1);
        }
        entries[index] = std::move(text);
    }

    return !entries.empty();
}

bool NameDictionary::loadSpecialNamesSox(const std::string& soxPath, const std::string& localizedPath) {
    auto soxData = readSoxFile(soxPath);
    if (soxData.size() < 8) return false;

    uint32_t version = readU32LE(soxData.data());
    uint32_t count = readU32LE(soxData.data() + 4);
    if (version != 100 || count == 0) return false;

    // SpecialNames.sox has a paired format: each record is
    // (uint16 key_len + key_bytes) + (uint16 default_len + default_bytes).
    struct RawEntry {
        std::vector<std::byte> keyBytes;
        std::string defaultName;
    };

    std::vector<RawEntry> rawEntries;
    rawEntries.reserve(count);

    size_t offset = 8;
    for (uint32_t i = 0; i < count; ++i) {
        if (offset + 2 > soxData.size()) break;
        if (isThendMarker(soxData.data() + offset, soxData.size() - offset)) break;

        uint16_t keyLen = readU16LE(soxData.data() + offset);
        offset += 2;
        if (offset + keyLen > soxData.size()) break;

        std::vector<std::byte> keyBytes(soxData.data() + offset, soxData.data() + offset + keyLen);
        offset += keyLen;

        if (offset + 2 > soxData.size()) break;
        uint16_t defaultLen = readU16LE(soxData.data() + offset);
        offset += 2;

        std::string defaultName;
        if (defaultLen > 0 && offset + defaultLen <= soxData.size()) {
            defaultName = std::string(reinterpret_cast<const char*>(soxData.data() + offset), defaultLen);
        }
        offset += defaultLen;

        rawEntries.push_back({std::move(keyBytes), std::move(defaultName)});
    }

    // Load localized display names from SpecialNames_ENG.sox.
    // This file has a simple non-indexed format: one string per entry.
    std::vector<std::string> displayNames;
    auto locData = readSoxFile(localizedPath);
    if (locData.size() >= 8) {
        uint32_t locVersion = readU32LE(locData.data());
        uint32_t locCount = readU32LE(locData.data() + 4);
        if (locVersion == 100 && locCount > 0) {
            size_t locOffset = 8;
            for (uint32_t i = 0; i < locCount; ++i) {
                if (locOffset + 2 > locData.size()) break;
                if (isThendMarker(locData.data() + locOffset, locData.size() - locOffset)) break;

                uint16_t slen = readU16LE(locData.data() + locOffset);
                locOffset += 2;
                if (locOffset + slen > locData.size()) break;

                displayNames.emplace_back(reinterpret_cast<const char*>(locData.data() + locOffset), slen);
                locOffset += slen;
            }
        }
    }

    specialNames_.clear();
    specialNames_.reserve(rawEntries.size());
    for (size_t i = 0; i < rawEntries.size(); ++i) {
        SpecialNameEntry entry;
        entry.keyBytes = std::move(rawEntries[i].keyBytes);
        if (i < displayNames.size() && !displayNames[i].empty()) {
            entry.displayName = std::move(displayNames[i]);
        } else {
            entry.displayName = std::move(rawEntries[i].defaultName);
        }
        specialNames_.push_back(std::move(entry));
    }

    return !specialNames_.empty();
}

bool NameDictionary::load(const std::string& soxDir) {
    if (soxDir.empty()) return false;

    fs::path base(soxDir);
    fs::path engDir = base / "ENG";

    // Load TroopInfo_ENG.sox — names for standard job types 0-42.
    fs::path troopEngPath = engDir / "TroopInfo_ENG.sox";
    if (fs::exists(troopEngPath)) {
        loadIndexedTextSox(troopEngPath.string(), troopInfoNames_);
    }

    // Load CharInfo_ENG.sox — names for character types (heroes, special units).
    fs::path charInfoPath = engDir / "CharInfo_ENG.sox";
    if (fs::exists(charInfoPath)) {
        loadIndexedTextSox(charInfoPath.string(), charInfoNames_);
    }

    // Load SpecialNames paired format for prefix-match name resolution.
    fs::path specialSoxPath = base / "SpecialNames.sox";
    fs::path specialEngPath = engDir / "SpecialNames_ENG.sox";
    if (fs::exists(specialSoxPath)) {
        loadSpecialNamesSox(specialSoxPath.string(), specialEngPath.string());
    }

    // Build Korean→English translation map from SpecialNames data.
    for (const auto& entry : specialNames_) {
        if (entry.keyBytes.empty() || entry.displayName.empty()) continue;

        std::string rawKey(reinterpret_cast<const char*>(entry.keyBytes.data()), entry.keyBytes.size());
        std::string korClean = stripDelimiters(rawKey);
        std::string korUtf8 = cp949ToUtf8(korClean);
        std::string engClean = stripDelimiters(entry.displayName);

        if (!korUtf8.empty() && !engClean.empty()) {
            koreanToEnglish_[korUtf8] = engClean;
        }
    }

    loaded_ = !troopInfoNames_.empty() || !charInfoNames_.empty() || !specialNames_.empty();
    return loaded_;
}

const char* NameDictionary::troopInfoName(uint32_t index) const {
    if (index < troopInfoNames_.size() && !troopInfoNames_[index].empty()) {
        return troopInfoNames_[index].c_str();
    }
    return nullptr;
}

const char* NameDictionary::charInfoName(uint8_t jobType) const {
    if (jobType < charInfoNames_.size() && !charInfoNames_[jobType].empty()) {
        return charInfoNames_[jobType].c_str();
    }
    return nullptr;
}

std::string NameDictionary::translate(const std::string& korean) const {
    if (korean.empty()) return {};

    // Strip "--" delimiters from input (STG unit names can include them).
    std::string cleaned = stripDelimiters(korean);
    if (cleaned.empty()) cleaned = korean;

    // Exact match.
    auto it = koreanToEnglish_.find(cleaned);
    if (it != koreanToEnglish_.end()) {
        return it->second;
    }

    // Strip trailing digits and try again.
    std::string base = stripTrailingDigits(cleaned);
    if (!base.empty()) {
        it = koreanToEnglish_.find(base);
        if (it != koreanToEnglish_.end()) {
            return it->second;
        }
    }

    return {};
}

std::string findGameDirectory(const std::string& stgFilePath) {
    fs::path stgPath(stgFilePath);
    fs::path dir = stgPath.parent_path();

    // Walk up from the STG file's directory looking for a sibling SOX/ folder.
    for (int depth = 0; depth < 5; ++depth) {
        if (dir.empty()) break;

        fs::path soxDir = dir / "SOX";
        if (fs::is_directory(soxDir)) {
            return soxDir.string();
        }

        fs::path parent = dir.parent_path();
        if (parent == dir) break;
        dir = parent;
    }

    return {};
}

} // namespace kuf
