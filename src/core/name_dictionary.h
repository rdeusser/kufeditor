#pragma once

#include <array>
#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>

namespace kuf {

struct SpecialNameEntry {
    std::vector<std::byte> keyBytes;
    std::string displayName;
};

struct WeaponVariant {
    int id;
    std::string shortName;
    std::string longName;
};

struct ItemAttEntry {
    std::string name;
    std::string description;
};

class NameDictionary {
public:
    bool load(const std::string& soxDir);

    const char* troopInfoName(uint32_t index) const;
    const char* charInfoName(uint8_t jobType) const;
    const char* leaderGenName(uint32_t poolIndex, int32_t nameIndex) const;
    const std::vector<SpecialNameEntry>& specialNames() const { return specialNames_; }

    std::string weaponName(int32_t itemTypeId, uint16_t variantIndex, int16_t enhancementTier) const;
    size_t itemTypeCount() const { return weaponNames_.size(); }
    size_t itemAttCount() const { return itemAttNames_.size(); }
    std::string itemTypeBaseName(int32_t itemTypeId) const;
    const char* itemAttName(int32_t attrIndex) const;
    const char* itemAttDescription(int32_t attrIndex) const;

    std::string translate(const std::string& korean) const;
    std::string reverseTranslate(const std::string& english) const;

    bool loaded() const { return loaded_; }

private:
    bool loadIndexedTextSox(const std::string& path, std::vector<std::string>& entries);
    bool loadSpecialNamesSox(const std::string& soxPath, const std::string& localizedPath);
    bool loadWeaponNames(const std::string& textDir);
    bool loadItemAttInfo(const std::string& soxEngDir);
    bool loadItemTypePrefixes(const std::string& soxEngDir);
    std::vector<std::byte> readSoxFile(const std::string& path);

    std::vector<std::string> troopInfoNames_;
    std::vector<std::string> charInfoNames_;
    std::vector<std::vector<std::string>> leaderGenPools_;
    std::vector<SpecialNameEntry> specialNames_;
    std::unordered_map<std::string, std::string> koreanToEnglish_;
    std::unordered_map<std::string, std::string> englishToKorean_;
    std::vector<std::vector<WeaponVariant>> weaponNames_;
    std::vector<ItemAttEntry> itemAttNames_;
    std::vector<std::array<std::string, 3>> itemTypePrefixes_;
    bool loaded_ = false;
};

std::string findGameDirectory(const std::string& stgFilePath);

} // namespace kuf
