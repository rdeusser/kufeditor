#include "formats/sox_binary.h"

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
constexpr size_t TROOP_RECORD_SIZE = 148;
constexpr size_t FOOTER_SIZE = 64;

} // namespace

bool SoxBinary::load(std::span<const std::byte> data) {
    if (data.size() < HEADER_SIZE) {
        return false;
    }

    headerVersion_ = readLE<int32_t>(data.data());
    int32_t count = readLE<int32_t>(data.data() + 4);

    if (headerVersion_ != 100) {
        return false;
    }

    size_t expectedSize = HEADER_SIZE + (count * TROOP_RECORD_SIZE) + FOOTER_SIZE;
    if (data.size() < expectedSize) {
        return false;
    }

    troops_.clear();
    troops_.reserve(count);

    const std::byte* ptr = data.data() + HEADER_SIZE;
    for (int32_t i = 0; i < count; ++i) {
        TroopInfo troop{};
        troop.job = readLE<int32_t>(ptr + 0x00);
        troop.typeId = readLE<int32_t>(ptr + 0x04);
        troop.moveSpeed = readLE<float>(ptr + 0x08);
        troop.rotateRate = readLE<float>(ptr + 0x0C);
        troop.moveAcceleration = readLE<float>(ptr + 0x10);
        troop.moveDeceleration = readLE<float>(ptr + 0x14);
        troop.sightRange = readLE<float>(ptr + 0x18);
        troop.attackRangeMax = readLE<float>(ptr + 0x1C);
        troop.attackRangeMin = readLE<float>(ptr + 0x20);
        troop.attackFrontRange = readLE<float>(ptr + 0x24);
        troop.directAttack = readLE<float>(ptr + 0x28);
        troop.indirectAttack = readLE<float>(ptr + 0x2C);
        troop.defense = readLE<float>(ptr + 0x30);
        troop.baseWidth = readLE<float>(ptr + 0x34);
        troop.resistMelee = readLE<float>(ptr + 0x38);
        troop.resistRanged = readLE<float>(ptr + 0x3C);
        troop.resistFrontal = readLE<float>(ptr + 0x40);
        troop.resistExplosion = readLE<float>(ptr + 0x44);
        troop.resistFire = readLE<float>(ptr + 0x48);
        troop.resistIce = readLE<float>(ptr + 0x4C);
        troop.resistLightning = readLE<float>(ptr + 0x50);
        troop.resistHoly = readLE<float>(ptr + 0x54);
        troop.resistCurse = readLE<float>(ptr + 0x58);
        troop.resistPoison = readLE<float>(ptr + 0x5C);
        troop.maxUnitSpeedMultiplier = readLE<float>(ptr + 0x60);
        troop.defaultUnitHp = readLE<float>(ptr + 0x64);
        troop.formationRandom = readLE<int32_t>(ptr + 0x68);
        troop.defaultUnitNumX = readLE<int32_t>(ptr + 0x6C);
        troop.defaultUnitNumY = readLE<int32_t>(ptr + 0x70);
        troop.unitHpLevelUp = readLE<float>(ptr + 0x74);

        for (int j = 0; j < 3; ++j) {
            troop.levelUpData[j].skillId = readLE<int32_t>(ptr + 0x78 + j * 8);
            troop.levelUpData[j].skillPerLevel = readLE<float>(ptr + 0x7C + j * 8);
        }

        troop.damageDistribution = readLE<float>(ptr + 0x90);

        troops_.push_back(troop);
        ptr += TROOP_RECORD_SIZE;
    }

    footer_.assign(ptr, ptr + FOOTER_SIZE);
    version_ = GameVersion::Crusaders;

    return true;
}

std::vector<std::byte> SoxBinary::save() const {
    std::vector<std::byte> data;
    data.resize(HEADER_SIZE + troops_.size() * TROOP_RECORD_SIZE + FOOTER_SIZE);

    std::byte* ptr = data.data();
    writeLE(ptr, headerVersion_);
    writeLE(ptr + 4, static_cast<int32_t>(troops_.size()));
    ptr += HEADER_SIZE;

    for (const auto& troop : troops_) {
        writeLE(ptr + 0x00, troop.job);
        writeLE(ptr + 0x04, troop.typeId);
        writeLE(ptr + 0x08, troop.moveSpeed);
        writeLE(ptr + 0x0C, troop.rotateRate);
        writeLE(ptr + 0x10, troop.moveAcceleration);
        writeLE(ptr + 0x14, troop.moveDeceleration);
        writeLE(ptr + 0x18, troop.sightRange);
        writeLE(ptr + 0x1C, troop.attackRangeMax);
        writeLE(ptr + 0x20, troop.attackRangeMin);
        writeLE(ptr + 0x24, troop.attackFrontRange);
        writeLE(ptr + 0x28, troop.directAttack);
        writeLE(ptr + 0x2C, troop.indirectAttack);
        writeLE(ptr + 0x30, troop.defense);
        writeLE(ptr + 0x34, troop.baseWidth);
        writeLE(ptr + 0x38, troop.resistMelee);
        writeLE(ptr + 0x3C, troop.resistRanged);
        writeLE(ptr + 0x40, troop.resistFrontal);
        writeLE(ptr + 0x44, troop.resistExplosion);
        writeLE(ptr + 0x48, troop.resistFire);
        writeLE(ptr + 0x4C, troop.resistIce);
        writeLE(ptr + 0x50, troop.resistLightning);
        writeLE(ptr + 0x54, troop.resistHoly);
        writeLE(ptr + 0x58, troop.resistCurse);
        writeLE(ptr + 0x5C, troop.resistPoison);
        writeLE(ptr + 0x60, troop.maxUnitSpeedMultiplier);
        writeLE(ptr + 0x64, troop.defaultUnitHp);
        writeLE(ptr + 0x68, troop.formationRandom);
        writeLE(ptr + 0x6C, troop.defaultUnitNumX);
        writeLE(ptr + 0x70, troop.defaultUnitNumY);
        writeLE(ptr + 0x74, troop.unitHpLevelUp);

        for (int j = 0; j < 3; ++j) {
            writeLE(ptr + 0x78 + j * 8, troop.levelUpData[j].skillId);
            writeLE(ptr + 0x7C + j * 8, troop.levelUpData[j].skillPerLevel);
        }

        writeLE(ptr + 0x90, troop.damageDistribution);
        ptr += TROOP_RECORD_SIZE;
    }

    std::memcpy(ptr, footer_.data(), footer_.size());

    return data;
}

std::vector<ValidationIssue> SoxBinary::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < troops_.size(); ++i) {
        const auto& troop = troops_[i];

        auto checkResistance = [&](float value, const char* name) {
            if (value < 0.0f || value > 2.0f) {
                issues.push_back({
                    Severity::Warning,
                    name,
                    "Resistance outside normal range (0.0-2.0)",
                    i
                });
            }
        };

        checkResistance(troop.resistMelee, "resistMelee");
        checkResistance(troop.resistRanged, "resistRanged");
        checkResistance(troop.resistFrontal, "resistFrontal");
        checkResistance(troop.resistExplosion, "resistExplosion");
        checkResistance(troop.resistFire, "resistFire");
        checkResistance(troop.resistIce, "resistIce");
        checkResistance(troop.resistLightning, "resistLightning");
        checkResistance(troop.resistHoly, "resistHoly");
        checkResistance(troop.resistCurse, "resistCurse");
        checkResistance(troop.resistPoison, "resistPoison");

        if (troop.defaultUnitHp <= 0) {
            issues.push_back({
                Severity::Error,
                "defaultUnitHp",
                "HP must be positive",
                i
            });
        }
    }

    return issues;
}

} // namespace kuf
