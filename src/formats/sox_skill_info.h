#pragma once

#include "formats/file_format.h"

#include <cstdint>
#include <string>
#include <vector>

namespace kuf {

struct SkillInfo {
    int32_t id;
    std::string locKey;
    std::string iconPath;
    uint32_t skillType;
    uint32_t maxLevel;
};

class SoxSkillInfo : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "SkillInfo SOX"; }
    GameVersion detectedVersion() const override { return version_; }
    std::vector<ValidationIssue> validate() const override;

    int32_t version() const { return headerVersion_; }
    size_t recordCount() const { return skills_.size(); }
    const std::vector<SkillInfo>& skills() const { return skills_; }
    std::vector<SkillInfo>& skills() { return skills_; }

private:
    int32_t headerVersion_ = 0;
    std::vector<SkillInfo> skills_;
    GameVersion version_ = GameVersion::Unknown;
    std::vector<std::byte> footer_;
};

} // namespace kuf
