#include "formats/stg_format.h"

#include "core/text_encoding.h"

#include <algorithm>
#include <cstring>

namespace kuf {

namespace {

template<typename T>
T readLE(const std::byte* data) {
    T value;
    std::memcpy(&value, data, sizeof(T));
    return value;
}

template<typename T>
void writeLE(std::byte* data, T value) {
    std::memcpy(data, &value, sizeof(T));
}

// Read a null-terminated string from a fixed-size buffer.
std::string readFixedString(const std::byte* data, size_t maxLen) {
    const char* str = reinterpret_cast<const char*>(data);
    size_t len = strnlen(str, maxLen);
    return std::string(str, len);
}

// Write a string into a fixed-size buffer, zero-padding the rest.
void writeFixedString(std::byte* data, size_t maxLen, const std::string& str) {
    std::memset(data, 0, maxLen);
    size_t copyLen = std::min(str.size(), maxLen - 1);
    std::memcpy(data, str.data(), copyLen);
}

} // namespace

void StgFormat::parseHeader(const std::byte* data) {
    std::memcpy(header_.rawData.data(), data, kStgHeaderSize);

    header_.missionId = readLE<uint32_t>(data + 0x000);
    header_.mapFile = readFixedString(data + 0x048, 64);
    header_.bitmapFile = readFixedString(data + 0x088, 64);
    header_.defaultCameraFile = readFixedString(data + 0x0C8, 64);
    header_.userCameraFile = readFixedString(data + 0x108, 64);
    header_.settingsFile = readFixedString(data + 0x148, 64);
    header_.skyCloudEffects = readFixedString(data + 0x188, 64);
    header_.aiScriptFile = readFixedString(data + 0x1C8, 64);
    header_.cubemapTexture = readFixedString(data + 0x20C, 64);
    header_.unitCount = readLE<uint32_t>(data + 0x270);
}

void StgFormat::patchHeader() const {
    std::byte* raw = const_cast<std::byte*>(header_.rawData.data());

    writeLE(raw + 0x000, header_.missionId);
    writeFixedString(raw + 0x048, 64, header_.mapFile);
    writeFixedString(raw + 0x088, 64, header_.bitmapFile);
    writeFixedString(raw + 0x0C8, 64, header_.defaultCameraFile);
    writeFixedString(raw + 0x108, 64, header_.userCameraFile);
    writeFixedString(raw + 0x148, 64, header_.settingsFile);
    writeFixedString(raw + 0x188, 64, header_.skyCloudEffects);
    writeFixedString(raw + 0x1C8, 64, header_.aiScriptFile);
    writeFixedString(raw + 0x20C, 64, header_.cubemapTexture);
    writeLE(raw + 0x270, header_.unitCount);
}

void StgFormat::parseUnit(StgUnit& unit, const std::byte* data) {
    std::memcpy(unit.rawData.data(), data, kStgUnitSize);

    // Core unit data (84 bytes starting at offset 0x00).
    unit.unitName = cp949ToUtf8(readFixedString(data + 0x00, 32));
    unit.uniqueId = readLE<uint32_t>(data + 0x20);
    unit.ucd = static_cast<UCD>(static_cast<uint8_t>(data[0x24]));
    unit.isHero = static_cast<uint8_t>(data[0x25]);
    unit.isEnabled = static_cast<uint8_t>(data[0x26]);
    unit.leaderHpOverride = readLE<float>(data + 0x28);
    unit.unitHpOverride = readLE<float>(data + 0x2C);
    unit.positionX = readLE<float>(data + 0x44);
    unit.positionY = readLE<float>(data + 0x48);
    unit.direction = static_cast<Direction>(static_cast<uint8_t>(data[0x4C]));

    // Leader configuration (108 bytes starting at offset 0x54).
    unit.leaderAnimationId = static_cast<uint8_t>(data[0x54]);
    unit.leaderModelVariant = static_cast<uint8_t>(data[0x55]);
    unit.leaderWorldmapId = static_cast<uint8_t>(data[0x56]);
    unit.leaderLevel = static_cast<uint8_t>(data[0x57]);

    // Skill slots (8 bytes at offset 0x58).
    for (int i = 0; i < 4; ++i) {
        unit.leaderSkills[i].skillId = static_cast<uint8_t>(data[0x58 + i * 2]);
        unit.leaderSkills[i].level = static_cast<uint8_t>(data[0x59 + i * 2]);
    }

    // Ability slots (92 bytes = 23 x int32 at offset 0x60).
    for (int i = 0; i < 23; ++i) {
        unit.leaderAbilities[i] = readLE<int32_t>(data + 0x60 + i * 4);
    }

    // Officer count at offset 0xBC.
    unit.officerCount = readLE<uint32_t>(data + 0xBC);

    // Officer 1 data (starts at offset 0xC0).
    unit.officer1.animationId = static_cast<uint8_t>(data[0xC0]);
    unit.officer1.modelVariant = static_cast<uint8_t>(data[0xC1]);
    unit.officer1.worldmapId = static_cast<uint8_t>(data[0xC2]);
    unit.officer1.level = static_cast<uint8_t>(data[0xC3]);
    for (int i = 0; i < 4; ++i) {
        unit.officer1.skills[i].skillId = static_cast<uint8_t>(data[0xC4 + i * 2]);
        unit.officer1.skills[i].level = static_cast<uint8_t>(data[0xC5 + i * 2]);
    }
    // Officer 1 abilities start after the 4 skill slots (8 bytes) + remaining data.
    // Officer 1 block is 104 bytes total from 0xC0 to 0x128.
    // After job/model/worldmap/level (4 bytes) + skills (8 bytes) = 0xCC.
    // Remaining 92 bytes = 23 abilities.
    for (int i = 0; i < 23; ++i) {
        unit.officer1.abilities[i] = readLE<int32_t>(data + 0xCC + i * 4);
    }

    // Officer 2 data (starts at offset 0x128).
    unit.officer2.animationId = static_cast<uint8_t>(data[0x128]);
    unit.officer2.modelVariant = static_cast<uint8_t>(data[0x129]);
    unit.officer2.worldmapId = static_cast<uint8_t>(data[0x12A]);
    unit.officer2.level = static_cast<uint8_t>(data[0x12B]);
    for (int i = 0; i < 4; ++i) {
        unit.officer2.skills[i].skillId = static_cast<uint8_t>(data[0x12C + i * 2]);
        unit.officer2.skills[i].level = static_cast<uint8_t>(data[0x12D + i * 2]);
    }
    // Officer 2 block is 88 bytes total from 0x128 to 0x180.
    // After job/model/worldmap/level (4 bytes) + skills (8 bytes) = 0x134.
    // Remaining 76 bytes = 19 abilities (not 23 - smaller block).
    for (int i = 0; i < 19; ++i) {
        unit.officer2.abilities[i] = readLE<int32_t>(data + 0x134 + i * 4);
    }

    // Unit configuration (160 bytes starting at offset 0x180).
    unit.gridUnk190 = readLE<uint32_t>(data + 0x190);
    unit.gridX = readLE<uint32_t>(data + 0x194);
    unit.gridY = readLE<uint32_t>(data + 0x198);
    unit.troopInfoIndex = readLE<uint32_t>(data + 0x1C0);
    unit.formationType = readLE<uint32_t>(data + 0x1C4);

    // Stat overrides: 22 floats at offset 0x1C8.
    for (int i = 0; i < 22; ++i) {
        unit.statOverrides[i] = readLE<float>(data + 0x1C8 + i * 4);
    }
}

void StgFormat::patchUnit(StgUnit& unit) const {
    std::byte* raw = unit.rawData.data();

    // Core unit data.
    writeFixedString(raw + 0x00, 32, utf8ToCp949(unit.unitName));
    writeLE(raw + 0x20, unit.uniqueId);
    raw[0x24] = static_cast<std::byte>(unit.ucd);
    raw[0x25] = static_cast<std::byte>(unit.isHero);
    raw[0x26] = static_cast<std::byte>(unit.isEnabled);
    writeLE(raw + 0x28, unit.leaderHpOverride);
    writeLE(raw + 0x2C, unit.unitHpOverride);
    writeLE(raw + 0x44, unit.positionX);
    writeLE(raw + 0x48, unit.positionY);
    raw[0x4C] = static_cast<std::byte>(unit.direction);

    // Leader configuration.
    raw[0x54] = static_cast<std::byte>(unit.leaderAnimationId);
    raw[0x55] = static_cast<std::byte>(unit.leaderModelVariant);
    raw[0x56] = static_cast<std::byte>(unit.leaderWorldmapId);
    raw[0x57] = static_cast<std::byte>(unit.leaderLevel);

    for (int i = 0; i < 4; ++i) {
        raw[0x58 + i * 2] = static_cast<std::byte>(unit.leaderSkills[i].skillId);
        raw[0x59 + i * 2] = static_cast<std::byte>(unit.leaderSkills[i].level);
    }

    for (int i = 0; i < 23; ++i) {
        writeLE(raw + 0x60 + i * 4, unit.leaderAbilities[i]);
    }

    writeLE(raw + 0xBC, unit.officerCount);

    // Officer 1.
    raw[0xC0] = static_cast<std::byte>(unit.officer1.animationId);
    raw[0xC1] = static_cast<std::byte>(unit.officer1.modelVariant);
    raw[0xC2] = static_cast<std::byte>(unit.officer1.worldmapId);
    raw[0xC3] = static_cast<std::byte>(unit.officer1.level);
    for (int i = 0; i < 4; ++i) {
        raw[0xC4 + i * 2] = static_cast<std::byte>(unit.officer1.skills[i].skillId);
        raw[0xC5 + i * 2] = static_cast<std::byte>(unit.officer1.skills[i].level);
    }
    for (int i = 0; i < 23; ++i) {
        writeLE(raw + 0xCC + i * 4, unit.officer1.abilities[i]);
    }

    // Officer 2.
    raw[0x128] = static_cast<std::byte>(unit.officer2.animationId);
    raw[0x129] = static_cast<std::byte>(unit.officer2.modelVariant);
    raw[0x12A] = static_cast<std::byte>(unit.officer2.worldmapId);
    raw[0x12B] = static_cast<std::byte>(unit.officer2.level);
    for (int i = 0; i < 4; ++i) {
        raw[0x12C + i * 2] = static_cast<std::byte>(unit.officer2.skills[i].skillId);
        raw[0x12D + i * 2] = static_cast<std::byte>(unit.officer2.skills[i].level);
    }
    for (int i = 0; i < 19; ++i) {
        writeLE(raw + 0x134 + i * 4, unit.officer2.abilities[i]);
    }

    // Unit configuration.
    writeLE(raw + 0x190, unit.gridUnk190);
    writeLE(raw + 0x194, unit.gridX);
    writeLE(raw + 0x198, unit.gridY);
    writeLE(raw + 0x1C0, unit.troopInfoIndex);
    writeLE(raw + 0x1C4, unit.formationType);

    for (int i = 0; i < 22; ++i) {
        writeLE(raw + 0x1C8 + i * 4, unit.statOverrides[i]);
    }
}

bool StgFormat::load(std::span<const std::byte> data) {
    if (data.size() < kStgHeaderSize) {
        return false;
    }

    parseHeader(data.data());

    uint32_t count = header_.unitCount;
    size_t expectedMinSize = kStgHeaderSize + count * kStgUnitSize;
    if (data.size() < expectedMinSize) {
        return false;
    }

    units_.clear();
    units_.resize(count);

    const std::byte* ptr = data.data() + kStgHeaderSize;
    for (uint32_t i = 0; i < count; ++i) {
        parseUnit(units_[i], ptr);
        ptr += kStgUnitSize;
    }

    // Everything after units is the raw tail (AreaIDs, Variables, Events, Footer).
    size_t tailOffset = kStgHeaderSize + count * kStgUnitSize;
    if (tailOffset < data.size()) {
        rawTail_.assign(data.begin() + tailOffset, data.end());
    } else {
        rawTail_.clear();
    }

    version_ = GameVersion::Crusaders;
    return true;
}

std::vector<std::byte> StgFormat::save() const {
    // Patch modified fields into raw buffers.
    patchHeader();
    for (auto& unit : const_cast<std::vector<StgUnit>&>(units_)) {
        patchUnit(unit);
    }

    std::vector<std::byte> data;
    data.reserve(kStgHeaderSize + units_.size() * kStgUnitSize + rawTail_.size());

    // Write header from raw data.
    data.insert(data.end(), header_.rawData.begin(), header_.rawData.end());

    // Write units from raw data.
    for (const auto& unit : units_) {
        data.insert(data.end(), unit.rawData.begin(), unit.rawData.end());
    }

    // Write raw tail (AreaIDs, Variables, Events, Footer).
    data.insert(data.end(), rawTail_.begin(), rawTail_.end());

    return data;
}

std::vector<ValidationIssue> StgFormat::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < units_.size(); ++i) {
        const auto& unit = units_[i];

        if (unit.unitName.empty()) {
            issues.push_back({
                Severity::Warning,
                "unitName",
                "Unit has no name",
                i
            });
        }

        if (static_cast<uint8_t>(unit.ucd) > 3) {
            issues.push_back({
                Severity::Error,
                "ucd",
                "Invalid UCD value",
                i
            });
        }

        if (unit.leaderLevel == 0 || unit.leaderLevel > 99) {
            issues.push_back({
                Severity::Warning,
                "leaderLevel",
                "Level outside typical range (1-99)",
                i
            });
        }

        if (unit.leaderWorldmapId != 0xFF && unit.leaderWorldmapId > 20) {
            issues.push_back({
                Severity::Warning,
                "leaderWorldmapId",
                "Worldmap ID may cause post-mission issues",
                i
            });
        }

        // Check for duplicate unique IDs.
        for (size_t j = i + 1; j < units_.size(); ++j) {
            if (units_[j].uniqueId == unit.uniqueId) {
                issues.push_back({
                    Severity::Error,
                    "uniqueId",
                    "Duplicate unique ID: " + std::to_string(unit.uniqueId),
                    i
                });
                break;
            }
        }

        // Validate officer count.
        if (unit.officerCount > 2) {
            issues.push_back({
                Severity::Error,
                "officerCount",
                "Officer count exceeds maximum of 2",
                i
            });
        }
    }

    return issues;
}

} // namespace kuf
