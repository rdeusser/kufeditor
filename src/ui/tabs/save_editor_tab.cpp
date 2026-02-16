#include "ui/tabs/save_editor_tab.h"
#include "ui/imgui_helpers.h"

#include <imgui.h>

#include <algorithm>
#include <cstring>

namespace kuf {

namespace {

const char* campaignNames[] = {
    "Hironeiden (Gerald)",
    "Vellond (Lucretia)",
    "Ecclesia (Kendal)",
    "Dark Legion (Regnier)"
};

const char* ucdNames[] = {"Player", "Enemy", "Ally", "Neutral"};

struct JobTypeEntry {
    uint32_t id;
    const char* name;
};

constexpr JobTypeEntry kJobTypeNames[] = {
    {0, "H_ARCHER"}, {1, "H_LONGBOW_MAN"}, {2, "H_INFANTRY"}, {3, "H_SPEARMAN"},
    {4, "H_H_INFANTRY"}, {5, "H_KNIGHT"}, {6, "H_PALADIN"}, {7, "H_CAVALRY"},
    {8, "H_H_CAVALRY"}, {9, "H_STORM_RIDER"}, {10, "H_SAPPER"},
    {11, "H_PYRO_TECHNICIAN"}, {12, "H_BOMBER_WING"}, {13, "H_MORTAR"},
    {14, "H_BALLISTA"}, {15, "H_HARPOON"}, {16, "H_CATAPULT"}, {17, "H_BATTALOON"},
    {18, "DE_ARCHER"}, {19, "DE_CAVALRY_ARCHER"}, {20, "DE_FIGHTER"},
    {21, "DE_KNIGHT"}, {22, "DE_LIGHT_CAVALRY"}, {23, "DO_INFANTRY"},
    {24, "DO_RIDER"}, {25, "DO_H_A_RIDERS"}, {26, "DO_AXE_MAN"},
    {27, "DO_H_A_INFANTRY"}, {28, "DO_SAPPER"}, {29, "D_SCORPION"},
    {30, "D_SWAMP_MAMMOTH"}, {31, "D_DIRIGIBLE"}, {32, "D_BLACK_WYVERN"},
    {33, "DO_GHOUL"}, {34, "D_BONE_DRAGON"}, {35, "WALL"}, {36, "SCOUT"},
    {37, "SELFDESTRUCTION"}, {38, "ENCABLOSA_MONSTER"},
    {39, "ENCABLOSA_FLYING_MONSTER"}, {40, "ENCABLOSA_RANGED"},
    {41, "ELF_WALL"}, {42, "ENCABLOSA_LARGE"},
};

constexpr size_t kJobTypeCount = sizeof(kJobTypeNames) / sizeof(kJobTypeNames[0]);

struct CharInfoEntry {
    uint32_t id;
    const char* name;
};

constexpr CharInfoEntry kCharInfoNames[] = {
    {32, "Gerald"}, {33, "Rupert"}, {34, "Regnier"}, {35, "Morene"},
    {36, "Thomas"}, {37, "Kendal"}, {38, "Ellen"}, {43, "Lucretia"},
    {44, "Leinhart"}, {45, "Urukubarr"}, {46, "Cirith"}, {47, "Valdemar"},
    {48, "Rithrin"}, {49, "Krawl"}, {53, "Lancelot"}, {54, "Lich"},
    {55, "Rick Miner"}, {56, "Leader"}, {57, "Dark Elf Leader"},
    {58, "Dark Elf Leader F"}, {59, "Ogre Leader"},
};

constexpr size_t kCharInfoCount = sizeof(kCharInfoNames) / sizeof(kCharInfoNames[0]);

// The job_type field is dual-purpose. The game uses the hero flag
// (runtime 0x59, save offset 57) to disambiguate:
//   is_hero == 0  → hero character → resolve through CharInfo (GetCharInfoName 0x00558de0)
//   is_hero != 0  → standard troop → resolve through K2_JOB_TYPE enum
//
// Without the flag, IDs like 32 would show "D_BLACK_WYVERN" instead of "Gerald".
const char* jobTypeName(uint32_t jobType, bool isHero) {
    if (isHero) {
        for (size_t i = 0; i < kCharInfoCount; ++i) {
            if (kCharInfoNames[i].id == jobType) return kCharInfoNames[i].name;
        }
    }
    for (size_t i = 0; i < kJobTypeCount; ++i) {
        if (kJobTypeNames[i].id == jobType) return kJobTypeNames[i].name;
    }
    for (size_t i = 0; i < kCharInfoCount; ++i) {
        if (kCharInfoNames[i].id == jobType) return kCharInfoNames[i].name;
    }
    return nullptr;
}

ImVec4 ucdColor(uint32_t ucd) {
    switch (ucd) {
        case 0: return ImVec4(0.2f, 0.8f, 0.2f, 1.0f);  // Player
        case 1: return ImVec4(0.9f, 0.2f, 0.2f, 1.0f);  // Enemy
        case 2: return ImVec4(0.2f, 0.5f, 0.9f, 1.0f);  // Ally
        case 3: return ImVec4(0.7f, 0.7f, 0.7f, 1.0f);  // Neutral
    }
    return ImVec4(1.0f, 1.0f, 1.0f, 1.0f);
}

const char* abilitySetLabels[] = {
    "Leader Set A", "Officer1 Set A", "Officer2 Set A",
    "Leader Set B", "Officer1 Set B", "Officer2 Set B",
};

} // namespace

SaveEditorTab::SaveEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {}

void SaveEditorTab::drawContent() {
    if (!document_ || !document_->saveData) {
        ImGui::TextDisabled("No save data loaded");
        return;
    }

    float totalHeight = ImGui::GetContentRegionAvail().y;

    ImGui::BeginChild("SaveSidebar", ImVec2(120, totalHeight), ImGuiChildFlags_Borders);
    drawSidebar();
    ImGui::EndChild();

    ImGui::SameLine();

    switch (currentSection_) {
    case Section::Summary:
        ImGui::BeginChild("SaveSummaryContent", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        drawSummarySection();
        ImGui::EndChild();
        break;

    case Section::Units:
        ImGui::BeginChild("SaveUnitList", ImVec2(250, totalHeight), ImGuiChildFlags_Borders);
        drawUnitList();
        ImGui::EndChild();

        ImGui::SameLine();

        ImGui::BeginChild("SaveUnitDetails", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        if (selectedUnit_ >= 0 &&
            selectedUnit_ < static_cast<int>(document_->saveData->unitCount())) {
            drawUnitDetails(selectedUnit_);
        } else {
            ImGui::TextDisabled("Select a unit to edit");
        }
        ImGui::EndChild();
        break;

    case Section::Roster:
        ImGui::BeginChild("SaveRosterContent", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        drawRosterSection();
        ImGui::EndChild();
        break;

    case Section::Missions:
        ImGui::BeginChild("SaveMissionsContent", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        drawMissionsSection();
        ImGui::EndChild();
        break;
    }
}

void SaveEditorTab::drawSidebar() {
    auto* save = document_->saveData.get();

    ImGui::Text("Sections");
    ImGui::Separator();

    if (ImGui::Selectable("Summary", currentSection_ == Section::Summary)) {
        currentSection_ = Section::Summary;
    }

    char unitsLabel[32];
    snprintf(unitsLabel, sizeof(unitsLabel), "Units (%zu)", save->unitCount());
    if (ImGui::Selectable(unitsLabel, currentSection_ == Section::Units)) {
        currentSection_ = Section::Units;
    }

    char rosterLabel[32];
    snprintf(rosterLabel, sizeof(rosterLabel), "Roster (%zu)", save->roster().size());
    if (ImGui::Selectable(rosterLabel, currentSection_ == Section::Roster)) {
        currentSection_ = Section::Roster;
    }

    if (ImGui::Selectable("Missions", currentSection_ == Section::Missions)) {
        currentSection_ = Section::Missions;
    }
}

void SaveEditorTab::drawSummarySection() {
    auto* save = document_->saveData.get();

    ImGui::Text("Save Game Summary");
    ImGui::Separator();

    // Campaign dropdown.
    int campaign = save->campaignIndex();
    if (campaign >= 0 && campaign <= 3) {
        if (ComboCentered("Campaign", &campaign, campaignNames, 4)) {
            save->setCampaignIndex(campaign);
            document_->dirty = true;
        }
    } else {
        ImGui::Text("Campaign: Unknown (%d)", campaign);
    }

    ImGui::Separator();

    // Main block fields.
    if (ImGui::CollapsingHeader("Map / File References", ImGuiTreeNodeFlags_DefaultOpen)) {
        auto& mb = save->mainBlock();
        char buf[64];

        auto stringInput = [&](const char* label, std::string& str) {
            std::memset(buf, 0, sizeof(buf));
            std::strncpy(buf, str.c_str(), sizeof(buf) - 1);
            if (InputTextCentered(label, buf, sizeof(buf))) {
                str = buf;
                document_->dirty = true;
            }
        };

        stringInput("Map Name", mb.mapName);
        stringInput("Set File", mb.setFile);
        stringInput("Sky Effects", mb.skyEffects);
    }

    if (ImGui::CollapsingHeader("Header Fields")) {
        auto& mb = save->mainBlock();

        auto fieldInput = [&](const char* label, uint32_t& val) {
            int v = static_cast<int>(val);
            if (ImGui::DragInt(label, &v, 1, 0, 0)) {
                val = static_cast<uint32_t>(v);
                document_->dirty = true;
            }
        };

        auto signedFieldInput = [&](const char* label, int32_t& val) {
            if (ImGui::DragInt(label, &val, 1, 0, 0)) {
                document_->dirty = true;
            }
        };

        fieldInput("Field 0x00", mb.field00);
        fieldInput("Field 0x04", mb.field04);
        signedFieldInput("Field 0x08", mb.field08);
        fieldInput("Field 0x0C", mb.field0c);
        fieldInput("Field 0x10", mb.field10);
        fieldInput("Field 0x14", mb.field14);
        fieldInput("Field 0x18", mb.field18);
    }

    ImGui::Separator();

    int selUnit = save->selectedUnit();
    if (ImGui::DragInt("Selected Unit", &selUnit, 1, -1, static_cast<int>(save->unitCount()) - 1)) {
        save->setSelectedUnit(selUnit);
        document_->dirty = true;
    }

    ImGui::Text("Units: %zu | Roster: %zu", save->unitCount(), save->roster().size());

    if (save->hasContext()) {
        if (ImGui::CollapsingHeader("Context Display Text")) {
            const auto& ctx = save->context();
            for (const auto& line : ctx.displayText) {
                ImGui::TextWrapped("%s", line.c_str());
            }
        }
    }
}

void SaveEditorTab::drawUnitList() {
    const auto& units = document_->saveData->units();

    for (size_t i = 0; i < units.size(); ++i) {
        const auto& unit = units[i];
        bool selected = (selectedUnit_ == static_cast<int>(i));

        ImVec4 color = ucdColor(unit.ucd);
        ImGui::PushStyleColor(ImGuiCol_Text, color);

        bool hero = unit.isHero == 0;
        const char* jtName = jobTypeName(unit.jobType, hero);
        char label[96];
        if (jtName) {
            snprintf(label, sizeof(label), "[%zu] %s (CharID=%d)", i, jtName, unit.charId);
        } else {
            snprintf(label, sizeof(label), "[%zu] Job %u (CharID=%d)", i, unit.jobType, unit.charId);
        }

        if (ImGui::Selectable(label, selected)) {
            selectedUnit_ = static_cast<int>(i);
        }

        ImGui::PopStyleColor();

        if (ImGui::IsItemHovered()) {
            const char* ucdName = (unit.ucd < 4) ? ucdNames[unit.ucd] : "Unknown";
            ImGui::SetTooltip("UCD: %s | Lv%u | TroopIdx: %d/%d | Hero: %s",
                ucdName, unit.skillLevel,
                unit.troopInfoIndex, unit.troopInfoIndex2,
                unit.isHero == 0 ? "Yes" : "No");
        }
    }
}

void SaveEditorTab::drawUnitDetails(size_t index) {
    auto& unit = document_->saveData->units()[index];

    bool hero = unit.isHero == 0;
    const char* jtName = jobTypeName(unit.jobType, hero);
    if (jtName) {
        ImGui::Text("[%zu] %s", index, jtName);
    } else {
        ImGui::Text("[%zu] Job %u", index, unit.jobType);
    }
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Core", ImGuiTreeNodeFlags_DefaultOpen)) {
        // Job type combo.
        {
            const char* currentName = jobTypeName(unit.jobType, hero);
            char preview[64];
            if (currentName) {
                snprintf(preview, sizeof(preview), "%s (%u)", currentName, unit.jobType);
            } else {
                snprintf(preview, sizeof(preview), "Job %u", unit.jobType);
            }

            if (BeginComboCentered("Job Type", preview)) {
                for (size_t i = 0; i < kJobTypeCount; ++i) {
                    char itemLabel[64];
                    snprintf(itemLabel, sizeof(itemLabel), "%s (%u)", kJobTypeNames[i].name, kJobTypeNames[i].id);
                    bool sel = (unit.jobType == kJobTypeNames[i].id);
                    if (ImGui::Selectable(itemLabel, sel)) {
                        unit.jobType = kJobTypeNames[i].id;
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::Separator();
                for (size_t i = 0; i < kCharInfoCount; ++i) {
                    char itemLabel[64];
                    snprintf(itemLabel, sizeof(itemLabel), "%s (%u)", kCharInfoNames[i].name, kCharInfoNames[i].id);
                    bool sel = (unit.jobType == kCharInfoNames[i].id);
                    if (ImGui::Selectable(itemLabel, sel)) {
                        unit.jobType = kCharInfoNames[i].id;
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::EndCombo();
            }
        }

        int modelId = static_cast<int>(unit.modelId);
        if (ImGui::DragInt("Model ID", &modelId, 1, 0, 0)) {
            unit.modelId = static_cast<uint32_t>(std::max(0, modelId));
            document_->dirty = true;
        }

        int charId = unit.charId;
        if (ImGui::DragInt("Char ID", &charId, 1, 0, 0)) {
            unit.charId = charId;
            document_->dirty = true;
        }

        int troopIdx = unit.troopInfoIndex;
        if (ImGui::DragInt("TroopInfo Index", &troopIdx, 1, 0, 0)) {
            unit.troopInfoIndex = troopIdx;
            document_->dirty = true;
        }

        int troopIdx2 = unit.troopInfoIndex2;
        if (ImGui::DragInt("TroopInfo Index 2", &troopIdx2, 1, 0, 0)) {
            unit.troopInfoIndex2 = troopIdx2;
            document_->dirty = true;
        }

        // UCD dropdown.
        int ucdIdx = static_cast<int>(unit.ucd);
        if (ucdIdx >= 0 && ucdIdx <= 3) {
            if (ComboCentered("UCD", &ucdIdx, ucdNames, 4)) {
                unit.ucd = static_cast<uint32_t>(ucdIdx);
                document_->dirty = true;
            }
        } else {
            ImGui::Text("UCD: %u (Unknown)", unit.ucd);
        }

        int skillLv = static_cast<int>(unit.skillLevel);
        if (ImGui::DragInt("Skill Level", &skillLv, 1, 0, 65535)) {
            unit.skillLevel = static_cast<uint32_t>(std::max(0, skillLv));
            document_->dirty = true;
        }

        bool hero = unit.isHero == 0;
        if (ImGui::Checkbox("Is Hero", &hero)) {
            unit.isHero = hero ? 0 : 1;
            document_->dirty = true;
        }
        if (ImGui::IsItemHovered()) ImGui::SetTooltip("0 = Hero, non-zero = Troop");

        int b58 = unit.byte58;
        if (ImGui::DragInt("Byte 0x58", &b58, 1, 0, 255)) {
            unit.byte58 = static_cast<uint8_t>(std::clamp(b58, 0, 255));
            document_->dirty = true;
        }

        int b5a = unit.byte5a;
        if (ImGui::DragInt("Byte 0x5A", &b5a, 1, 0, 255)) {
            unit.byte5a = static_cast<uint8_t>(std::clamp(b5a, 0, 255));
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Formation")) {
        int formation = static_cast<int>(unit.formationType);
        if (ImGui::DragInt("Formation Type", &formation, 1, 0, 0)) {
            unit.formationType = static_cast<uint32_t>(std::max(0, formation));
            document_->dirty = true;
        }

        int gridCfg = static_cast<int>(unit.gridConfig);
        if (ImGui::DragInt("Grid Config", &gridCfg, 1, 0, 0)) {
            unit.gridConfig = static_cast<uint32_t>(std::max(0, gridCfg));
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Equipment")) {
        for (int e = 0; e < 6; ++e) {
            ImGui::PushID(e);
            char label[32];
            snprintf(label, sizeof(label), "Slot %d", e);
            int val = unit.equipment[e];
            if (ImGui::DragInt(label, &val)) {
                unit.equipment[e] = val;
                document_->dirty = true;
            }
            ImGui::PopID();
        }
    }

    if (ImGui::CollapsingHeader("Ability Sets")) {
        drawAbilitySets(unit);
    }

    if (ImGui::CollapsingHeader("Unknown Fields")) {
        int unknIdx = unit.unknownIndex;
        if (ImGui::DragInt("Unknown Index", &unknIdx, 1, 0, 0)) {
            unit.unknownIndex = unknIdx;
            document_->dirty = true;
        }

        auto fieldU32 = [&](const char* label, uint32_t& val) {
            int v = static_cast<int>(val);
            if (ImGui::DragInt(label, &v, 1, 0, 0)) {
                val = static_cast<uint32_t>(v);
                document_->dirty = true;
            }
        };

        fieldU32("STG Field 0x34", unit.stgField34);
        fieldU32("STG Field 0x38", unit.stgField38);
        fieldU32("STG Field 0x3C", unit.stgField3c);
        fieldU32("STG Field 0x40", unit.stgField40);
        fieldU32("Field 0x60", unit.field60);
        fieldU32("Field 0x64", unit.field64);
        fieldU32("Field 0x68", unit.field68);
        fieldU32("Field 0x504", unit.field504);
    }
}

void SaveEditorTab::drawAbilitySets(SaveUnit& unit) {
    for (int s = 0; s < 6; ++s) {
        ImGui::PushID(s);

        // Count non-empty abilities for the label.
        int active = 0;
        for (int a = 0; a < 16; ++a) {
            if (unit.abilitySets[s][a] != -1 && unit.abilitySets[s][a] != 0) {
                ++active;
            }
        }

        char setLabel[64];
        if (active > 0) {
            snprintf(setLabel, sizeof(setLabel), "%s (%d active)", abilitySetLabels[s], active);
        } else {
            snprintf(setLabel, sizeof(setLabel), "%s (Empty)", abilitySetLabels[s]);
        }

        if (ImGui::TreeNode(setLabel)) {
            for (int a = 0; a < 16; ++a) {
                ImGui::PushID(a);
                int val = unit.abilitySets[s][a];

                char slotLabel[32];
                snprintf(slotLabel, sizeof(slotLabel), "Slot %d", a);

                if (val == -1) {
                    ImGui::TextDisabled("%s: Empty", slotLabel);
                    ImGui::SameLine();
                    if (ImGui::SmallButton("Set")) {
                        unit.abilitySets[s][a] = 0;
                        document_->dirty = true;
                    }
                } else {
                    ImGui::SetNextItemWidth(120);
                    if (ImGui::DragInt(slotLabel, &val)) {
                        unit.abilitySets[s][a] = val;
                        document_->dirty = true;
                    }
                    ImGui::SameLine();
                    if (ImGui::SmallButton("Clear")) {
                        unit.abilitySets[s][a] = -1;
                        document_->dirty = true;
                    }
                }
                ImGui::PopID();
            }
            ImGui::TreePop();
        }
        ImGui::PopID();
    }
}

void SaveEditorTab::drawRosterSection() {
    auto& roster = document_->saveData->roster();

    ImGui::Text("Roster (%zu entries)", roster.size());
    ImGui::Separator();

    if (roster.empty()) {
        ImGui::TextDisabled("No roster entries");
        return;
    }

    if (ImGui::BeginTable("RosterTable", 6,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg | ImGuiTableFlags_ScrollY)) {
        ImGui::TableSetupColumn("#", ImGuiTableColumnFlags_WidthFixed, 40.0f);
        ImGui::TableSetupColumn("Byte 60", ImGuiTableColumnFlags_WidthFixed, 80.0f);
        ImGui::TableSetupColumn("Byte 61", ImGuiTableColumnFlags_WidthFixed, 80.0f);
        ImGui::TableSetupColumn("Byte 62", ImGuiTableColumnFlags_WidthFixed, 80.0f);
        ImGui::TableSetupColumn("Byte 63", ImGuiTableColumnFlags_WidthFixed, 80.0f);
        ImGui::TableSetupColumn("Value 64", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < roster.size(); ++i) {
            ImGui::PushID(static_cast<int>(i));
            ImGui::TableNextRow();

            ImGui::TableSetColumnIndex(0);
            ImGui::Text("%zu", i);

            ImGui::TableSetColumnIndex(1);
            int b60 = roster[i].byte60;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b60", &b60, 1, 0, 255)) {
                roster[i].byte60 = static_cast<uint8_t>(std::clamp(b60, 0, 255));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(2);
            int b61 = roster[i].byte61;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b61", &b61, 1, 0, 255)) {
                roster[i].byte61 = static_cast<uint8_t>(std::clamp(b61, 0, 255));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(3);
            int b62 = roster[i].byte62;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b62", &b62, 1, 0, 255)) {
                roster[i].byte62 = static_cast<uint8_t>(std::clamp(b62, 0, 255));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(4);
            int b63 = roster[i].byte63;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b63", &b63, 1, 0, 255)) {
                roster[i].byte63 = static_cast<uint8_t>(std::clamp(b63, 0, 255));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(5);
            int val = static_cast<int>(roster[i].value64);
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##val", &val, 1, 0, 0)) {
                roster[i].value64 = static_cast<uint32_t>(val);
                document_->dirty = true;
            }

            ImGui::PopID();
        }

        ImGui::EndTable();
    }
}

void SaveEditorTab::drawMissionsSection() {
    auto* save = document_->saveData.get();

    ImGui::Text("Mission Progress");
    ImGui::Separator();

    int missionIdx = save->currentMissionIndex();
    if (ImGui::DragInt("Current Mission Index", &missionIdx, 1, 0, 0)) {
        save->setCurrentMissionIndex(missionIdx);
        document_->dirty = true;
    }

    ImGui::Separator();
    ImGui::Text("Mission Completion (20 slots)");

    auto& completion = save->missionCompletion();

    if (ImGui::BeginTable("MissionTable", 4,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg)) {
        ImGui::TableSetupColumn("Slot", ImGuiTableColumnFlags_WidthFixed, 60.0f);
        ImGui::TableSetupColumn("Value", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableSetupColumn("Slot", ImGuiTableColumnFlags_WidthFixed, 60.0f);
        ImGui::TableSetupColumn("Value", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < kSaveMissionSlots; i += 2) {
            ImGui::TableNextRow();

            ImGui::TableSetColumnIndex(0);
            ImGui::Text("[%zu]", i);

            ImGui::TableSetColumnIndex(1);
            ImGui::PushID(static_cast<int>(i));
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##mc", &completion[i], 1, 0, 0)) {
                document_->dirty = true;
            }
            ImGui::PopID();

            if (i + 1 < kSaveMissionSlots) {
                ImGui::TableSetColumnIndex(2);
                ImGui::Text("[%zu]", i + 1);

                ImGui::TableSetColumnIndex(3);
                ImGui::PushID(static_cast<int>(i + 1));
                ImGui::SetNextItemWidth(-1);
                if (ImGui::DragInt("##mc", &completion[i + 1], 1, 0, 0)) {
                    document_->dirty = true;
                }
                ImGui::PopID();
            }
        }

        ImGui::EndTable();
    }

    // Second array display.
    auto& secondArr = save->secondArray();
    if (!secondArr.empty()) {
        ImGui::Separator();
        if (ImGui::CollapsingHeader("Second Array")) {
            for (size_t i = 0; i < secondArr.size(); ++i) {
                ImGui::PushID(static_cast<int>(i) + 1000);
                char label[32];
                snprintf(label, sizeof(label), "[%zu]", i);
                int val = static_cast<int>(secondArr[i]);
                if (ImGui::DragInt(label, &val, 1, 0, 0)) {
                    secondArr[i] = static_cast<uint32_t>(val);
                    document_->dirty = true;
                }
                ImGui::PopID();
            }
        }
    }
}

} // namespace kuf
