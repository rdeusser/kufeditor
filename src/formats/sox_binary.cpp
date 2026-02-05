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

// Read int32 and convert to float (file stores integers, not IEEE floats).
float readIntAsFloat(const std::byte* data) {
    return static_cast<float>(readLE<int32_t>(data));
}

// Write float as int32 (reverse of readIntAsFloat).
void writeFloatAsInt(std::byte* data, float value) {
    writeLE(data, static_cast<int32_t>(value));
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
        troop.moveSpeed = readIntAsFloat(ptr + 0x08);
        troop.rotateRate = readIntAsFloat(ptr + 0x0C);
        troop.moveAcceleration = readIntAsFloat(ptr + 0x10);
        troop.moveDeceleration = readIntAsFloat(ptr + 0x14);
        troop.sightRange = readIntAsFloat(ptr + 0x18);
        troop.attackRangeMax = readIntAsFloat(ptr + 0x1C);
        troop.attackRangeMin = readIntAsFloat(ptr + 0x20);
        troop.attackFrontRange = readIntAsFloat(ptr + 0x24);
        troop.directAttack = readIntAsFloat(ptr + 0x28);
        troop.indirectAttack = readIntAsFloat(ptr + 0x2C);
        troop.defense = readIntAsFloat(ptr + 0x30);
        troop.baseWidth = readIntAsFloat(ptr + 0x34);
        troop.resistMelee = readIntAsFloat(ptr + 0x38);
        troop.resistRanged = readIntAsFloat(ptr + 0x3C);
        troop.resistFrontal = readIntAsFloat(ptr + 0x40);
        troop.resistExplosion = readIntAsFloat(ptr + 0x44);
        troop.resistFire = readIntAsFloat(ptr + 0x48);
        troop.resistIce = readIntAsFloat(ptr + 0x4C);
        troop.resistLightning = readIntAsFloat(ptr + 0x50);
        troop.resistHoly = readIntAsFloat(ptr + 0x54);
        troop.resistCurse = readIntAsFloat(ptr + 0x58);
        troop.resistPoison = readIntAsFloat(ptr + 0x5C);
        troop.maxUnitSpeedMultiplier = readIntAsFloat(ptr + 0x60);
        troop.defaultUnitHp = readIntAsFloat(ptr + 0x64);
        troop.formationRandom = readLE<int32_t>(ptr + 0x68);
        troop.defaultUnitNumX = readLE<int32_t>(ptr + 0x6C);
        troop.defaultUnitNumY = readLE<int32_t>(ptr + 0x70);
        troop.unitHpLevelUp = readIntAsFloat(ptr + 0x74);

        for (int j = 0; j < 3; ++j) {
            troop.levelUpData[j].skillId = readLE<int32_t>(ptr + 0x78 + j * 8);
            troop.levelUpData[j].bonusPerLevel = readIntAsFloat(ptr + 0x7C + j * 8);
        }

        troop.damageDistribution = readIntAsFloat(ptr + 0x90);

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
        writeFloatAsInt(ptr + 0x08, troop.moveSpeed);
        writeFloatAsInt(ptr + 0x0C, troop.rotateRate);
        writeFloatAsInt(ptr + 0x10, troop.moveAcceleration);
        writeFloatAsInt(ptr + 0x14, troop.moveDeceleration);
        writeFloatAsInt(ptr + 0x18, troop.sightRange);
        writeFloatAsInt(ptr + 0x1C, troop.attackRangeMax);
        writeFloatAsInt(ptr + 0x20, troop.attackRangeMin);
        writeFloatAsInt(ptr + 0x24, troop.attackFrontRange);
        writeFloatAsInt(ptr + 0x28, troop.directAttack);
        writeFloatAsInt(ptr + 0x2C, troop.indirectAttack);
        writeFloatAsInt(ptr + 0x30, troop.defense);
        writeFloatAsInt(ptr + 0x34, troop.baseWidth);
        writeFloatAsInt(ptr + 0x38, troop.resistMelee);
        writeFloatAsInt(ptr + 0x3C, troop.resistRanged);
        writeFloatAsInt(ptr + 0x40, troop.resistFrontal);
        writeFloatAsInt(ptr + 0x44, troop.resistExplosion);
        writeFloatAsInt(ptr + 0x48, troop.resistFire);
        writeFloatAsInt(ptr + 0x4C, troop.resistIce);
        writeFloatAsInt(ptr + 0x50, troop.resistLightning);
        writeFloatAsInt(ptr + 0x54, troop.resistHoly);
        writeFloatAsInt(ptr + 0x58, troop.resistCurse);
        writeFloatAsInt(ptr + 0x5C, troop.resistPoison);
        writeFloatAsInt(ptr + 0x60, troop.maxUnitSpeedMultiplier);
        writeFloatAsInt(ptr + 0x64, troop.defaultUnitHp);
        writeLE(ptr + 0x68, troop.formationRandom);
        writeLE(ptr + 0x6C, troop.defaultUnitNumX);
        writeLE(ptr + 0x70, troop.defaultUnitNumY);
        writeFloatAsInt(ptr + 0x74, troop.unitHpLevelUp);

        for (int j = 0; j < 3; ++j) {
            writeLE(ptr + 0x78 + j * 8, troop.levelUpData[j].skillId);
            writeFloatAsInt(ptr + 0x7C + j * 8, troop.levelUpData[j].bonusPerLevel);
        }

        writeFloatAsInt(ptr + 0x90, troop.damageDistribution);
        ptr += TROOP_RECORD_SIZE;
    }

    std::memcpy(ptr, footer_.data(), footer_.size());

    return data;
}

std::vector<ValidationIssue> SoxBinary::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < troops_.size(); ++i) {
        const auto& troop = troops_[i];

        // Resistances: 0=immune, 100=normal, 250+=very vulnerable, 1000000+=instant death.
        // Only flag negative values or extremely high non-instant-death values.
        auto checkResistance = [&](float value, const char* name) {
            int v = static_cast<int>(value);
            if (v < 0 || (v > 500 && v < 1000000)) {
                issues.push_back({
                    Severity::Warning,
                    name,
                    "Resistance outside typical range",
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
