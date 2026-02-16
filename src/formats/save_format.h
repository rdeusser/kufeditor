#pragma once

#include "formats/file_format.h"

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace kuf {

static constexpr uint32_t kSaveMagic = 0x6E;
static constexpr size_t kSaveMainBlockSize = 0x154;
static constexpr size_t kSaveContextSize = 0x438;
static constexpr size_t kSaveUnitSize = 483;
static constexpr size_t kSaveRosterRecordSize = 8;
static constexpr size_t kSavePadTarget = 0x8000;
static constexpr size_t kSaveMissionSlots = 20;

struct SaveContext {
    std::array<std::byte, kSaveContextSize> rawData{};
    std::vector<std::string> displayText;
};

struct SaveMainBlock {
    std::array<std::byte, kSaveMainBlockSize> rawData{};

    uint32_t field00 = 0;
    uint32_t field04 = 0;
    int32_t field08 = 0;
    uint32_t field0c = 0;
    uint32_t field10 = 0;
    uint32_t field14 = 0;
    uint32_t field18 = 0;

    std::string mapName;
    std::string setFile;
    std::string skyEffects;
};

struct SaveUnit {
    int32_t unknownIndex = 0;
    int32_t troopInfoIndex = 0;
    uint32_t jobType = 0;
    uint32_t modelId = 0;
    uint32_t stgField34 = 0;
    uint32_t stgField38 = 0;
    uint32_t stgField3c = 0;
    uint32_t stgField40 = 0;
    int32_t charId = 0;
    int32_t troopInfoIndex2 = 0;
    uint32_t ucd = 0;
    uint32_t formationType = 0;
    uint32_t gridConfig = 0;
    uint32_t skillLevel = 0;
    uint8_t byte58 = 0;
    uint8_t isHero = 0;
    uint8_t byte5a = 0;
    uint32_t field60 = 0;
    uint32_t field64 = 0;
    uint32_t field68 = 0;
    std::array<int32_t, 6> equipment{};
    std::array<std::array<int32_t, 16>, 6> abilitySets{};
    uint32_t field504 = 0;

    SaveUnit() {
        equipment.fill(-1);
        for (auto& set : abilitySets) {
            set.fill(-1);
        }
    }
};

struct SaveRosterRecord {
    uint8_t byte61 = 0;
    uint8_t byte60 = 0;
    uint8_t byte62 = 0;
    uint8_t byte63 = 0;
    uint32_t value64 = 0;
};

class SaveFormat : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "Save Game"; }
    GameVersion detectedVersion() const override { return GameVersion::Crusaders; }
    std::vector<ValidationIssue> validate() const override;

    bool hasSizePrefix() const { return hasSizePrefix_; }
    bool hasContext() const { return hasContext_; }

    const SaveContext& context() const { return context_; }

    int32_t campaignIndex() const { return campaignIndex_; }
    void setCampaignIndex(int32_t idx) { campaignIndex_ = idx; }

    const SaveMainBlock& mainBlock() const { return mainBlock_; }
    SaveMainBlock& mainBlock() { return mainBlock_; }

    size_t unitCount() const { return units_.size(); }
    const std::vector<SaveUnit>& units() const { return units_; }
    std::vector<SaveUnit>& units() { return units_; }

    int32_t selectedUnit() const { return selectedUnit_; }
    void setSelectedUnit(int32_t idx) { selectedUnit_ = idx; }

    const std::vector<SaveRosterRecord>& roster() const { return roster_; }
    std::vector<SaveRosterRecord>& roster() { return roster_; }

    const std::vector<uint32_t>& secondArray() const { return secondArray_; }
    std::vector<uint32_t>& secondArray() { return secondArray_; }

    const std::array<int32_t, kSaveMissionSlots>& missionCompletion() const { return missionCompletion_; }
    std::array<int32_t, kSaveMissionSlots>& missionCompletion() { return missionCompletion_; }

    int32_t currentMissionIndex() const { return currentMissionIndex_; }
    void setCurrentMissionIndex(int32_t idx) { currentMissionIndex_ = idx; }

private:
    void parseContext(const std::byte* data);
    void parseMainBlock(const std::byte* data);
    void patchMainBlock() const;

    bool hasSizePrefix_ = false;
    bool hasContext_ = false;
    SaveContext context_;
    int32_t campaignIndex_ = 0;
    SaveMainBlock mainBlock_;
    std::vector<SaveUnit> units_;
    int32_t selectedUnit_ = -1;
    std::vector<SaveRosterRecord> roster_;
    std::vector<uint32_t> secondArray_;
    std::array<int32_t, kSaveMissionSlots> missionCompletion_{};
    int32_t currentMissionIndex_ = 0;
    std::vector<std::byte> rawTail_;
};

} // namespace kuf
