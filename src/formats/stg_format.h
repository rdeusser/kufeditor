#pragma once

#include "formats/file_format.h"

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace kuf {

// Unit Control Disposition - controls AI behavior.
enum class UCD : uint8_t {
    Player  = 0,
    Enemy   = 1,
    Ally    = 2,
    Neutral = 3
};

// Facing direction (counter-clockwise from East).
enum class Direction : uint8_t {
    East      = 0,
    NorthEast = 1,
    North     = 2,
    NorthWest = 3,
    West      = 4,
    SouthWest = 5,
    South     = 6,
    SouthEast = 7
};

// K2JobDef.h animation IDs (0-42). Values above 42 are extended model IDs
// for hero characters and special unit animations.
constexpr uint8_t kMaxStandardAnimationId = 42;

// Skill slot: 1 byte skill ID + 1 byte level, packed into 2 bytes (4 slots = 8 bytes).
struct SkillSlot {
    uint8_t skillId = 0;
    uint8_t level = 0;
};

// Officer data within a unit block.
struct OfficerData {
    uint8_t animationId = 0;
    uint8_t modelVariant = 0;
    uint8_t worldmapId = 0xFF;
    uint8_t level = 1;
    std::array<SkillSlot, 4> skills{};
    std::array<int32_t, 23> abilities{};

    OfficerData() {
        abilities.fill(-1);
    }
};

// STG file header (628 bytes).
struct StgHeader {
    uint32_t missionId = 0;
    std::string mapFile;
    std::string bitmapFile;
    std::string defaultCameraFile;
    std::string userCameraFile;
    std::string settingsFile;
    std::string skyCloudEffects;
    std::string aiScriptFile;
    std::string cubemapTexture;
    uint32_t unitCount = 0;

    // Raw header bytes for round-trip fidelity.
    std::array<std::byte, 628> rawData{};
};

// STG unit block (544 bytes for Crusaders).
struct StgUnit {
    // Core unit data (84 bytes).
    std::string unitName;
    uint32_t uniqueId = 0;
    UCD ucd = UCD::Enemy;
    uint8_t isHero = 0;
    uint8_t isEnabled = 1;
    float leaderHpOverride = -1.0f;
    float unitHpOverride = -1.0f;
    float positionX = 0.0f;
    float positionY = 0.0f;
    Direction direction = Direction::East;

    // Leader configuration (108 bytes).
    uint8_t leaderAnimationId = 0;
    uint8_t leaderModelVariant = 0;
    uint8_t leaderWorldmapId = 0xFF;
    uint8_t leaderLevel = 1;
    std::array<SkillSlot, 4> leaderSkills{};
    std::array<int32_t, 23> leaderAbilities{};
    uint32_t officerCount = 0;

    // Officers.
    OfficerData officer1;
    OfficerData officer2;

    // Unit configuration (160 bytes).
    uint32_t troopInfoIndex = 0;
    uint32_t formationType = 0;
    uint32_t gridUnk190 = 1;
    uint32_t gridX = 1;
    uint32_t gridY = 1;
    std::array<float, 22> statOverrides{};

    // Raw unit bytes for round-trip fidelity.
    std::array<std::byte, 544> rawData{};

    StgUnit() {
        leaderAbilities.fill(-1);
        statOverrides.fill(-1.0f);
    }
};

static constexpr size_t kStgHeaderSize = 628;
static constexpr size_t kStgUnitSize = 544;

class StgFormat : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "STG Mission"; }
    GameVersion detectedVersion() const override { return version_; }
    std::vector<ValidationIssue> validate() const override;

    const StgHeader& header() const { return header_; }
    StgHeader& header() { return header_; }

    size_t unitCount() const { return units_.size(); }
    const std::vector<StgUnit>& units() const { return units_; }
    std::vector<StgUnit>& units() { return units_; }

private:
    void parseHeader(const std::byte* data);
    void patchHeader() const;
    void parseUnit(StgUnit& unit, const std::byte* data);
    void patchUnit(StgUnit& unit) const;

    StgHeader header_;
    std::vector<StgUnit> units_;
    std::vector<std::byte> rawTail_;
    GameVersion version_ = GameVersion::Unknown;
};

} // namespace kuf
