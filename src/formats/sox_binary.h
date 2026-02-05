#pragma once

#include "formats/file_format.h"

#include <array>
#include <cstdint>
#include <vector>

namespace kuf {

struct LevelUpData {
    int32_t skillId;
    float bonusPerLevel;
};

struct TroopInfo {
    int32_t job;
    int32_t typeId;
    float moveSpeed;
    float rotateRate;
    float moveAcceleration;
    float moveDeceleration;
    float sightRange;
    float attackRangeMax;
    float attackRangeMin;
    float attackFrontRange;
    float directAttack;
    float indirectAttack;
    float defense;
    float baseWidth;
    float resistMelee;
    float resistRanged;
    float resistFrontal;
    float resistExplosion;
    float resistFire;
    float resistIce;
    float resistLightning;
    float resistHoly;
    float resistCurse;
    float resistPoison;
    float maxUnitSpeedMultiplier;
    float defaultUnitHp;
    int32_t formationRandom;
    int32_t defaultUnitNumX;
    int32_t defaultUnitNumY;
    float unitHpLevelUp;
    std::array<LevelUpData, 3> levelUpData;
    float damageDistribution;
};

class SoxBinary : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "Binary SOX"; }
    GameVersion detectedVersion() const override { return version_; }
    std::vector<ValidationIssue> validate() const override;

    int32_t version() const { return headerVersion_; }
    size_t recordCount() const { return troops_.size(); }
    const std::vector<TroopInfo>& troops() const { return troops_; }
    std::vector<TroopInfo>& troops() { return troops_; }

private:
    int32_t headerVersion_ = 0;
    std::vector<TroopInfo> troops_;
    GameVersion version_ = GameVersion::Unknown;
    std::vector<std::byte> footer_;
};

} // namespace kuf
