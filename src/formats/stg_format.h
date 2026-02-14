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

// K2JobDef.h job type IDs (0-42). Values above 42 are extended model IDs
// for hero characters and special unit animations.
constexpr uint8_t kMaxStandardJobType = 42;

// Skill slot: 1 byte skill ID + 1 byte level, packed into 2 bytes (4 slots = 8 bytes).
struct SkillSlot {
    uint8_t skillId = 0;
    uint8_t level = 0;
};

// Officer data within a unit block.
struct OfficerData {
    uint8_t jobType = 0;
    uint8_t modelId = 0;
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
    uint32_t formatMagic = 0x3E9;
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
    uint8_t leaderJobType = 0;
    uint8_t leaderModelId = 0;
    uint8_t leaderWorldmapId = 0xFF;
    uint8_t leaderLevel = 1;
    std::array<SkillSlot, 4> leaderSkills{};
    std::array<int32_t, 23> leaderAbilities{};
    uint32_t officerCount = 0;

    // Officers.
    OfficerData officer1;
    OfficerData officer2;

    // Unit configuration (160 bytes).
    int32_t troopInfoIndex = 0;
    uint32_t formationType = 0;
    uint32_t unitAnimConfig = 0;
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
static constexpr size_t kStgAreaIdEntrySize = 84;
static constexpr size_t kStgEventDescriptionSize = 64;
static constexpr size_t kStgVariableNameSize = 64;

// Typed parameter value system (matches ReadSTGParamValue at 0x004847b0).
enum class StgParamType : uint32_t {
    Int    = 0,
    Float  = 1,
    String = 2,
    Enum   = 3
};

struct StgParamValue {
    StgParamType type = StgParamType::Int;
    int32_t intValue = 0;
    float floatValue = 0.0f;
    std::string stringValue;

    size_t serializedSize() const {
        if (type == StgParamType::String) {
            return 8 + stringValue.size();
        }
        return 8;
    }
};

struct StgScriptEntry {
    uint32_t typeId = 0;
    std::vector<StgParamValue> params;
};

struct StgEvent {
    std::string description;
    uint32_t eventId = 0;
    std::vector<StgScriptEntry> conditions;
    std::vector<StgScriptEntry> actions;
    std::vector<std::byte> rawData;
    bool modified = false;
};

struct StgEventBlock {
    uint32_t blockHeader = 0;
    std::vector<StgEvent> events;
};

struct StgVariable {
    std::string name;
    uint32_t variableId = 0;
    StgParamValue initialValue;
};

struct StgArea {
    std::string description;
    uint32_t areaId = 0;
    float boundX1 = 0.0f;
    float boundY1 = 0.0f;
    float boundX2 = 0.0f;
    float boundY2 = 0.0f;
    std::array<std::byte, 84> rawData{};
};

struct StgFooterEntry {
    uint32_t field1 = 0;
    uint32_t field2 = 0;
};

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

    const std::vector<StgArea>& areas() const { return areas_; }
    std::vector<StgArea>& areas() { return areas_; }

    const std::vector<StgEventBlock>& eventBlocks() const { return eventBlocks_; }
    std::vector<StgEventBlock>& eventBlocks() { return eventBlocks_; }

    const std::vector<StgVariable>& variables() const { return variables_; }
    std::vector<StgVariable>& variables() { return variables_; }

    const std::vector<StgFooterEntry>& footerEntries() const { return footerEntries_; }
    std::vector<StgFooterEntry>& footerEntries() { return footerEntries_; }

    size_t totalEventCount() const;
    bool tailParsed() const { return tailParsed_; }

private:
    void parseHeader(const std::byte* data);
    void patchHeader() const;
    void parseUnit(StgUnit& unit, const std::byte* data);
    void patchUnit(StgUnit& unit) const;
    bool parseTail(const std::byte* data, size_t tailSize);

    size_t parseAreaIds(const std::byte* data, size_t tailSize, size_t offset);
    size_t parseVariables(const std::byte* data, size_t tailSize, size_t offset);
    size_t parseEventBlocks(const std::byte* data, size_t tailSize, size_t offset);
    size_t parseFooter(const std::byte* data, size_t tailSize, size_t offset);
    StgParamValue readParamValue(const std::byte* data, size_t& offset, size_t limit) const;

    void serializeParamValue(std::vector<std::byte>& out, const StgParamValue& val) const;
    void serializeAreaIds(std::vector<std::byte>& out) const;
    void serializeVariables(std::vector<std::byte>& out) const;
    void serializeEventBlocks(std::vector<std::byte>& out) const;
    void serializeFooter(std::vector<std::byte>& out) const;

    StgHeader header_;
    std::vector<StgUnit> units_;
    std::vector<StgArea> areas_;
    std::vector<StgVariable> variables_;
    std::vector<StgEventBlock> eventBlocks_;
    std::vector<StgFooterEntry> footerEntries_;
    std::vector<std::byte> rawTail_;
    bool tailParsed_ = false;
    GameVersion version_ = GameVersion::Unknown;
};

} // namespace kuf
