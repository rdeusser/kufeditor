#include "core/name_dictionary.h"

#include "core/text_encoding.h"
#include "formats/sox_encoding.h"

#include <algorithm>
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

bool NameDictionary::loadNonIndexedTextSox(const std::string& path, std::vector<std::string>& entries) {
    auto data = readSoxFile(path);
    if (data.size() < 8) return false;

    uint32_t version = readU32LE(data.data());
    uint32_t count = readU32LE(data.data() + 4);
    if (version != 100 || count == 0) return false;

    size_t offset = 8;
    for (uint32_t i = 0; i < count; ++i) {
        // Skip null padding bytes.
        while (offset < data.size() && data[offset] == std::byte{0}) {
            ++offset;
        }

        // Check for THEND trailer.
        if (offset + 5 <= data.size()) {
            const char* p = reinterpret_cast<const char*>(data.data() + offset);
            if (std::strncmp(p, "THEND", 5) == 0) break;
        }

        if (offset + 2 > data.size()) break;

        uint16_t len = readU16LE(data.data() + offset);
        offset += 2;

        if (len == 0 || offset + len > data.size()) break;

        std::string text(reinterpret_cast<const char*>(data.data() + offset), len);
        offset += len;

        entries.push_back(std::move(text));
    }

    return !entries.empty();
}

bool NameDictionary::load(const std::string& soxDir) {
    if (soxDir.empty()) return false;

    fs::path base(soxDir);
    fs::path engDir = base / "ENG";

    // Load CharInfo_ENG.sox for character type names (covers IDs 0-62+).
    // This provides names for extended animation IDs (>42) like "Lich",
    // "Leader", "Dark Elf Leader" that TroopInfo_ENG doesn't cover.
    fs::path charInfoPath = engDir / "CharInfo_ENG.sox";
    if (fs::exists(charInfoPath)) {
        loadIndexedTextSox(charInfoPath.string(), troopNames_);
    }

    // Load TroopInfo_ENG.sox and overwrite entries 0-42 with its names.
    // TroopInfo has better names for standard troops ("Archer" vs "Empty Leader").
    fs::path troopEngPath = engDir / "TroopInfo_ENG.sox";
    if (fs::exists(troopEngPath)) {
        std::vector<std::string> troopEntries;
        if (loadIndexedTextSox(troopEngPath.string(), troopEntries)) {
            for (size_t i = 0; i < troopEntries.size(); ++i) {
                if (!troopEntries[i].empty()) {
                    if (i >= troopNames_.size()) {
                        troopNames_.resize(i + 1);
                    }
                    troopNames_[i] = troopEntries[i];
                }
            }
        }
    }

    // Load SpecialNames pairs for Korean→English translation.
    fs::path specialEngPath = engDir / "SpecialNames_ENG.sox";
    fs::path specialKorPath = base / "SpecialNames.sox";

    if (fs::exists(specialEngPath) && fs::exists(specialKorPath)) {
        std::vector<std::string> engNames;
        std::vector<std::string> korNames;

        loadNonIndexedTextSox(specialEngPath.string(), engNames);
        loadNonIndexedTextSox(specialKorPath.string(), korNames);

        size_t pairs = std::min(engNames.size(), korNames.size());
        for (size_t i = 0; i < pairs; ++i) {
            std::string korClean = stripDelimiters(korNames[i]);
            std::string korUtf8 = cp949ToUtf8(korClean);

            std::string engClean = stripDelimiters(engNames[i]);

            if (!korUtf8.empty() && !engClean.empty()) {
                koreanToEnglish_[korUtf8] = engClean;
            }
        }
    }

    loaded_ = !troopNames_.empty() || !koreanToEnglish_.empty();
    return loaded_;
}

const char* NameDictionary::troopName(uint8_t animId) const {
    if (animId < troopNames_.size() && !troopNames_[animId].empty()) {
        return troopNames_[animId].c_str();
    }
    return nullptr;
}

const char* NameDictionary::troopNameByIndex(uint32_t troopInfoIndex) const {
    if (troopInfoIndex < troopNames_.size() && !troopNames_[troopInfoIndex].empty()) {
        return troopNames_[troopInfoIndex].c_str();
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
