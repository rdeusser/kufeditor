#include "formats/sox_skill_info.h"

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

constexpr size_t HEADER_SIZE = 8;
constexpr size_t FOOTER_SIZE = 64;

} // namespace

bool SoxSkillInfo::load(std::span<const std::byte> data) {
    if (data.size() < HEADER_SIZE + FOOTER_SIZE) {
        return false;
    }

    headerVersion_ = readLE<int32_t>(data.data());
    int32_t count = readLE<int32_t>(data.data() + 4);

    if (headerVersion_ != 100 || count <= 0) {
        return false;
    }

    skills_.clear();
    skills_.reserve(count);

    const std::byte* ptr = data.data() + HEADER_SIZE;
    const std::byte* end = data.data() + data.size() - FOOTER_SIZE;

    for (int32_t i = 0; i < count; ++i) {
        SkillInfo skill{};

        if (ptr + sizeof(int32_t) > end) return false;
        skill.id = readLE<int32_t>(ptr);
        ptr += sizeof(int32_t);

        // Localization key: uint16 length + raw bytes.
        if (ptr + sizeof(uint16_t) > end) return false;
        uint16_t locLen = readLE<uint16_t>(ptr);
        ptr += sizeof(uint16_t);
        if (ptr + locLen > end) return false;
        skill.locKey.assign(reinterpret_cast<const char*>(ptr), locLen);
        ptr += locLen;

        // Icon path: uint16 length + raw bytes.
        if (ptr + sizeof(uint16_t) > end) return false;
        uint16_t iconLen = readLE<uint16_t>(ptr);
        ptr += sizeof(uint16_t);
        if (ptr + iconLen > end) return false;
        skill.iconPath.assign(reinterpret_cast<const char*>(ptr), iconLen);
        ptr += iconLen;

        // Slot count and max level.
        if (ptr + 2 * sizeof(uint32_t) > end) return false;
        skill.slotCount = readLE<uint32_t>(ptr);
        ptr += sizeof(uint32_t);
        skill.maxLevel = readLE<uint32_t>(ptr);
        ptr += sizeof(uint32_t);

        skills_.push_back(std::move(skill));
    }

    // After all records, remaining data before the footer must be zero.
    if (ptr != end) {
        return false;
    }

    footer_.assign(end, end + FOOTER_SIZE);
    version_ = GameVersion::Crusaders;

    return true;
}

std::vector<std::byte> SoxSkillInfo::save() const {
    // Calculate total size.
    size_t dataSize = HEADER_SIZE;
    for (const auto& skill : skills_) {
        dataSize += sizeof(int32_t);                            // id
        dataSize += sizeof(uint16_t) + skill.locKey.size();     // locKey
        dataSize += sizeof(uint16_t) + skill.iconPath.size();   // iconPath
        dataSize += sizeof(uint32_t);                           // slotCount
        dataSize += sizeof(uint32_t);                           // maxLevel
    }
    dataSize += FOOTER_SIZE;

    std::vector<std::byte> data(dataSize, std::byte{0});

    std::byte* ptr = data.data();
    writeLE(ptr, headerVersion_);
    writeLE(ptr + 4, static_cast<int32_t>(skills_.size()));
    ptr += HEADER_SIZE;

    for (const auto& skill : skills_) {
        writeLE(ptr, skill.id);
        ptr += sizeof(int32_t);

        writeLE(ptr, static_cast<uint16_t>(skill.locKey.size()));
        ptr += sizeof(uint16_t);
        std::memcpy(ptr, skill.locKey.data(), skill.locKey.size());
        ptr += skill.locKey.size();

        writeLE(ptr, static_cast<uint16_t>(skill.iconPath.size()));
        ptr += sizeof(uint16_t);
        std::memcpy(ptr, skill.iconPath.data(), skill.iconPath.size());
        ptr += skill.iconPath.size();

        writeLE(ptr, skill.slotCount);
        ptr += sizeof(uint32_t);
        writeLE(ptr, skill.maxLevel);
        ptr += sizeof(uint32_t);
    }

    std::memcpy(ptr, footer_.data(), footer_.size());

    return data;
}

std::vector<ValidationIssue> SoxSkillInfo::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < skills_.size(); ++i) {
        const auto& skill = skills_[i];

        if (skill.slotCount < 1 || skill.slotCount > 4) {
            issues.push_back({
                Severity::Warning,
                "slotCount",
                "Slot count outside typical range (1-4)",
                i
            });
        }

        if (skill.maxLevel == 0 || skill.maxLevel > 65535) {
            issues.push_back({
                Severity::Warning,
                "maxLevel",
                "Max level is 0 or exceeds 65535",
                i
            });
        }

        if (skill.locKey.empty()) {
            issues.push_back({
                Severity::Warning,
                "locKey",
                "Localization key is empty",
                i
            });
        }

        if (skill.iconPath.empty()) {
            issues.push_back({
                Severity::Warning,
                "iconPath",
                "Icon path is empty",
                i
            });
        }
    }

    return issues;
}

} // namespace kuf
