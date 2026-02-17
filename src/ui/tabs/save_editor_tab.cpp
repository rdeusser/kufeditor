#include "ui/tabs/save_editor_tab.h"
#include "ui/imgui_helpers.h"
#include "formats/stg_format.h"
#include "undo/set_field_command.h"

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

const char* ucdNames[] = {"Leader", "Officer 1", "Officer 2", "Troop"};

// Resolve a unit's display name using SOX data, mirroring the game's
// name resolution chain from ReadUnitArray:
//   leaderNameIndex < 0 → GetCharInfoName (hero/story character)
//   leaderNameIndex >= 0 → LeaderGeneration name pool (generated leader)
//   fallback → CharInfo/TroopInfo generic name
const char* resolveUnitName(const SaveUnit& unit, const NameDictionary& dict) {
    if (unit.leaderNameIndex < 0) {
        const char* name = dict.charInfoName(static_cast<uint8_t>(unit.jobType));
        if (name) return name;
    } else {
        const char* name = dict.leaderGenName(
            static_cast<uint32_t>(unit.troopInfoIndex), unit.leaderNameIndex);
        if (name) return name;
    }
    if (unit.jobType <= kMaxStandardJobType) {
        const char* name = dict.troopInfoName(unit.jobType);
        if (name) return name;
    }
    return dict.charInfoName(static_cast<uint8_t>(unit.jobType));
}

ImVec4 ucdColor(uint32_t ucd) {
    switch (ucd) {
        case 0: return ImVec4(0.2f, 0.8f, 0.2f, 1.0f);   // Leader — green
        case 1: return ImVec4(0.9f, 0.75f, 0.2f, 1.0f);   // Officer 1 — gold
        case 2: return ImVec4(0.9f, 0.75f, 0.2f, 1.0f);   // Officer 2 — gold
        case 3: return ImVec4(0.2f, 0.8f, 0.2f, 1.0f);    // Troop — green
    }
    return ImVec4(1.0f, 1.0f, 1.0f, 1.0f);
}

const char* equipSlotLabels[] = {
    "Leader Weapon", "Leader Accessory", "Leader Armor",
    "Troop Weapon", "Troop Accessory", "Troop Armor",
};

const char* skillTypeNames[] = {
    "Melee", "Range", "Frontal", "Riding", "Teamwork", "Scout",
    "Gunpowder", "Taming", "Fire", "Lightning", "Ice", "Holy",
    "Earth", "Curse", "Elemental"
};

const char* resistTypeNames[] = {
    "Melee", "Ranged", "Explosion", "Frontal", "Fire",
    "Lightning", "Ice", "Holy", "Poison", "Curse"
};

const char* skillTypeName(int32_t type) {
    if (type < 0 || type > 14) return nullptr;
    return skillTypeNames[type];
}

const char* resistTypeName(int32_t type) {
    if (type < 0 || type > 9) return nullptr;
    return resistTypeNames[type];
}

const char* ucdRoleName(uint32_t ucd) {
    switch (ucd) {
        case 0: return "Leader";
        case 1: return "Officer 1";
        case 2: return "Officer 2";
        case 3: return "Troop";
    }
    return nullptr;
}

} // namespace

SaveEditorTab::SaveEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {
    if (document_ && !document_->path.empty()) {
        std::string soxDir = findGameDirectory(document_->path);
        if (!soxDir.empty()) nameDictionary_.load(soxDir);
    }
}

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
            document_->undoStack->execute(
                makeSetFieldCommand(&save->campaignIndexRef(), static_cast<int32_t>(campaign), "Change campaign"));
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
                document_->undoStack->execute(
                    makeSetFieldCommand(&str, std::string(buf), std::string("Change ") + label));
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
                document_->undoStack->execute(
                    makeSetFieldCommand(&val, static_cast<uint32_t>(v), std::string("Change ") + label));
                document_->dirty = true;
            }
        };

        auto signedFieldInput = [&](const char* label, int32_t& val) {
            int32_t prev = val;
            if (ImGui::DragInt(label, &val, 1, 0, 0)) {
                int32_t newVal = val;
                val = prev;
                document_->undoStack->execute(
                    makeSetFieldCommand(&val, newVal, std::string("Change ") + label));
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
        document_->undoStack->execute(
            makeSetFieldCommand(&save->selectedUnitRef(), static_cast<int32_t>(selUnit), "Change selected unit"));
        document_->dirty = true;
    }

    {
        const auto& units = save->units();
        int counts[4] = {};
        for (const auto& u : units) {
            if (u.ucd < 4) ++counts[u.ucd];
        }
        ImGui::Text("%zu units (%d leaders, %d officers1, %d officers2, %d troops) | Roster: %zu",
            units.size(), counts[0], counts[1], counts[2], counts[3],
            save->roster().size());
    }

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

    ImGui::Checkbox("Player only", &showPlayerOnly_);
    ImGui::Separator();

    for (size_t i = 0; i < units.size(); ++i) {
        const auto& unit = units[i];
        bool isLeader = (unit.ucd == 0);
        bool isOfficer = (unit.ucd == 1 || unit.ucd == 2);
        bool isTroop = (unit.ucd == 3);

        // In player-only mode, show leaders, their officers, and standalone troops.
        if (showPlayerOnly_ && !isLeader && !isOfficer && !isTroop) continue;
        // Also skip officers whose leader isn't player — walk back to find leader.
        if (showPlayerOnly_ && isOfficer) {
            bool leaderIsPlayer = false;
            for (int j = static_cast<int>(i) - 1; j >= 0; --j) {
                if (units[j].ucd == 0) { leaderIsPlayer = true; break; }
                if (units[j].ucd != 1 && units[j].ucd != 2) break;
            }
            if (!leaderIsPlayer) continue;
        }

        bool selected = (selectedUnit_ == static_cast<int>(i));

        ImVec4 color = ucdColor(unit.ucd);
        ImGui::PushStyleColor(ImGuiCol_Text, color);

        const char* jtName = resolveUnitName(unit, nameDictionary_);
        char label[128];
        const char* roleSuffix = ucdRoleName(unit.ucd);

        if (isOfficer) {
            // Indent officers under their leader.
            if (jtName) {
                snprintf(label, sizeof(label), "  [%zu] %s (%s)", i, jtName, roleSuffix);
            } else {
                snprintf(label, sizeof(label), "  [%zu] Job %u (%s)", i, unit.jobType, roleSuffix);
            }
        } else if (isLeader || isTroop) {
            if (jtName) {
                snprintf(label, sizeof(label), "[%zu] %s (%s)", i, jtName, roleSuffix);
            } else {
                snprintf(label, sizeof(label), "[%zu] Job %u (%s)", i, unit.jobType, roleSuffix);
            }
        } else {
            if (jtName) {
                snprintf(label, sizeof(label), "[%zu] %s (CharID=%d)", i, jtName, unit.charId);
            } else {
                snprintf(label, sizeof(label), "[%zu] Job %u (CharID=%d)", i, unit.jobType, unit.charId);
            }
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

    const char* jtName = resolveUnitName(unit, nameDictionary_);
    if (jtName) {
        ImGui::Text("[%zu] %s", index, jtName);
    } else {
        ImGui::Text("[%zu] Job %u", index, unit.jobType);
    }
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Core", ImGuiTreeNodeFlags_DefaultOpen)) {
        // Job type combo — standard troop types (0-42), then CharInfo entries.
        {
            const char* currentName = resolveUnitName(unit, nameDictionary_);
            char preview[64];
            if (currentName) {
                snprintf(preview, sizeof(preview), "%s (%u)", currentName, unit.jobType);
            } else {
                snprintf(preview, sizeof(preview), "Job %u", unit.jobType);
            }

            if (BeginComboCentered("Job Type", preview)) {
                for (uint32_t i = 0; i <= kMaxStandardJobType; ++i) {
                    const char* name = nameDictionary_.troopInfoName(i);
                    char itemLabel[64];
                    if (name) {
                        snprintf(itemLabel, sizeof(itemLabel), "%s (%u)", name, i);
                    } else {
                        snprintf(itemLabel, sizeof(itemLabel), "Job %u", i);
                    }
                    bool sel = (unit.jobType == i);
                    if (ImGui::Selectable(itemLabel, sel)) {
                        document_->undoStack->execute(
                            makeSetFieldCommand(&unit.jobType, i, "Change job type"));
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }

                ImGui::Separator();

                for (uint32_t i = kMaxStandardJobType + 1; i < 256; ++i) {
                    const char* charName = nameDictionary_.charInfoName(static_cast<uint8_t>(i));
                    if (!charName) continue;
                    char itemLabel[64];
                    snprintf(itemLabel, sizeof(itemLabel), "%s (%u)", charName, i);
                    bool sel = (unit.jobType == i);
                    if (ImGui::Selectable(itemLabel, sel)) {
                        document_->undoStack->execute(
                            makeSetFieldCommand(&unit.jobType, i, "Change job type"));
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::EndCombo();
            }
        }

        int modelId = static_cast<int>(unit.modelId);
        if (ImGui::DragInt("Model ID", &modelId, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.modelId, static_cast<uint32_t>(std::max(0, modelId)), "Change model ID"));
            document_->dirty = true;
        }

        int charId = unit.charId;
        if (ImGui::DragInt("Char ID", &charId, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.charId, static_cast<int32_t>(charId), "Change char ID"));
            document_->dirty = true;
        }

        int troopIdx = unit.troopInfoIndex;
        if (ImGui::DragInt("TroopInfo Index", &troopIdx, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.troopInfoIndex, static_cast<int32_t>(troopIdx), "Change troop info index"));
            document_->dirty = true;
        }

        int troopIdx2 = unit.troopInfoIndex2;
        if (ImGui::DragInt("TroopInfo Index 2", &troopIdx2, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.troopInfoIndex2, static_cast<int32_t>(troopIdx2), "Change troop info index 2"));
            document_->dirty = true;
        }

        // UCD dropdown.
        int ucdIdx = static_cast<int>(unit.ucd);
        if (ucdIdx >= 0 && ucdIdx <= 3) {
            if (ComboCentered("UCD", &ucdIdx, ucdNames, 4)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&unit.ucd, static_cast<uint32_t>(ucdIdx), "Change UCD"));
                document_->dirty = true;
            }
        } else {
            ImGui::Text("UCD: %u (Unknown)", unit.ucd);
        }

        int skillLv = static_cast<int>(unit.skillLevel);
        if (ImGui::DragInt("Skill Level", &skillLv, 1, 0, 65535)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.skillLevel, static_cast<uint32_t>(std::max(0, skillLv)), "Change skill level"));
            document_->dirty = true;
        }

        bool heroFlag = unit.isHero == 0;
        if (ImGui::Checkbox("Is Hero", &heroFlag)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.isHero, static_cast<uint8_t>(heroFlag ? 0 : 1), "Toggle hero flag"));
            document_->dirty = true;
        }
        if (ImGui::IsItemHovered()) ImGui::SetTooltip("0 = Hero, non-zero = Troop");

        int b58 = unit.byte58;
        if (ImGui::DragInt("Byte 0x58", &b58, 1, 0, 255)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.byte58, static_cast<uint8_t>(std::clamp(b58, 0, 255)), "Change byte 0x58"));
            document_->dirty = true;
        }

        int b5a = unit.byte5a;
        if (ImGui::DragInt("Byte 0x5A", &b5a, 1, 0, 255)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.byte5a, static_cast<uint8_t>(std::clamp(b5a, 0, 255)), "Change byte 0x5A"));
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Formation")) {
        int formation = static_cast<int>(unit.formationType);
        if (ImGui::DragInt("Formation Type", &formation, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.formationType, static_cast<uint32_t>(std::max(0, formation)), "Change formation type"));
            document_->dirty = true;
        }

        int gridCfg = static_cast<int>(unit.gridConfig);
        if (ImGui::DragInt("Grid Config", &gridCfg, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.gridConfig, static_cast<uint32_t>(std::max(0, gridCfg)), "Change grid config"));
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Equipment", ImGuiTreeNodeFlags_DefaultOpen)) {
        for (int e = 0; e < 6; ++e) {
            ImGui::PushID(e);
            drawEquipmentSlot(equipSlotLabels[e], unit.equipSlots()[e], e);
            ImGui::PopID();
        }
    }

    if (ImGui::CollapsingHeader("Unknown Fields")) {
        int nameIdx = unit.leaderNameIndex;
        if (ImGui::DragInt("Leader Name Index", &nameIdx, 1, 0, 0)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&unit.leaderNameIndex, static_cast<int32_t>(nameIdx), "Change leader name index"));
            document_->dirty = true;
        }

        auto fieldU32 = [&](const char* label, uint32_t& val) {
            int v = static_cast<int>(val);
            if (ImGui::DragInt(label, &v, 1, 0, 0)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&val, static_cast<uint32_t>(v), std::string("Change ") + label));
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

void SaveEditorTab::drawEquipmentSlot(const char* label, SaveEquipmentSlot& slot, int slotIdx) {
    bool isEmpty = slot.empty();

    // Build display name.
    char headerLabel[256];
    if (isEmpty) {
        snprintf(headerLabel, sizeof(headerLabel), "%s (Empty)###equip%d", label, slotIdx);
    } else {
        std::string name = nameDictionary_.weaponName(slot.itemTypeId, slot.variantIndex, slot.enhancementTier);
        if (name.empty()) {
            snprintf(headerLabel, sizeof(headerLabel), "%s: Item %d, Lv%u###equip%d",
                label, slot.itemTypeId, slot.level, slotIdx);
        } else {
            // Add attribute suffix.
            std::string suffix;
            const char* att1 = nameDictionary_.itemAttName(slot.attribute1Index);
            const char* att2 = nameDictionary_.itemAttName(slot.attribute2Index);
            if (att1 && att2) {
                suffix = std::string(" of ") + att1 + " and " + att2;
            } else if (att1) {
                suffix = std::string(" of ") + att1;
            } else if (att2) {
                suffix = std::string(" of ") + att2;
            }

            snprintf(headerLabel, sizeof(headerLabel), "%s: %s%s, Lv%u###equip%d",
                label, name.c_str(), suffix.c_str(), slot.level, slotIdx);
        }
    }

    ImGuiTreeNodeFlags flags = ImGuiTreeNodeFlags_None;
    if (!ImGui::TreeNodeEx(headerLabel, flags)) return;

    if (isEmpty) {
        ImGui::TextDisabled("No item equipped");
        ImGui::TreePop();
        return;
    }

    // Attribute effect descriptions — shown first as summary.
    const char* desc1 = nameDictionary_.itemAttDescription(slot.attribute1Index);
    const char* desc2 = nameDictionary_.itemAttDescription(slot.attribute2Index);
    if (desc1 || desc2) {
        ImGui::TextColored(ImVec4(0.6f, 0.8f, 1.0f, 1.0f), "Effects:");
        ImGui::SameLine();
        if (desc1 && desc2) {
            ImGui::TextWrapped("%s; %s", desc1, desc2);
        } else if (desc1) {
            ImGui::TextWrapped("%s", desc1);
        } else {
            ImGui::TextWrapped("%s", desc2);
        }
    }

    // Core editable fields.
    {
        // Item Type ID dropdown.
        size_t typeCount = nameDictionary_.itemTypeCount();
        if (typeCount > 0) {
            std::string currentName = nameDictionary_.itemTypeBaseName(slot.itemTypeId);
            char preview[64];
            if (!currentName.empty()) {
                snprintf(preview, sizeof(preview), "%s (%d)", currentName.c_str(), slot.itemTypeId);
            } else {
                snprintf(preview, sizeof(preview), "Item %d", slot.itemTypeId);
            }

            if (ImGui::BeginCombo("Item Type", preview)) {
                for (int t = 0; t < static_cast<int>(typeCount); ++t) {
                    std::string name = nameDictionary_.itemTypeBaseName(t);
                    if (name.empty()) continue;
                    char itemLabel[64];
                    snprintf(itemLabel, sizeof(itemLabel), "%s (%d)", name.c_str(), t);
                    bool sel = (slot.itemTypeId == t);
                    if (ImGui::Selectable(itemLabel, sel)) {
                        document_->undoStack->execute(
                            makeSetFieldCommand(&slot.itemTypeId, static_cast<int32_t>(t), "Change item type"));
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::EndCombo();
            }
        } else {
            int itemType = slot.itemTypeId;
            if (ImGui::DragInt("Item Type", &itemType)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&slot.itemTypeId, static_cast<int32_t>(itemType), "Change item type"));
                document_->dirty = true;
            }
        }

        int level = slot.level;
        if (ImGui::DragInt("Level", &level, 1, 0, 65535)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.level, static_cast<uint16_t>(std::max(0, level)), "Change level"));
            document_->dirty = true;
        }

        int tier = slot.enhancementTier;
        if (ImGui::DragInt("Enhancement Tier", &tier, 1, -1, 2)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.enhancementTier, static_cast<int16_t>(tier), "Change enhancement tier"));
            document_->dirty = true;
        }

        int variant = slot.variantIndex;
        if (ImGui::DragInt("Variant Index", &variant, 1, 0, 65535)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.variantIndex, static_cast<uint16_t>(std::max(0, variant)), "Change variant index"));
            document_->dirty = true;
        }
    }

    // Skill type combos with bonus inputs.
    {
        auto skillCombo = [&](const char* comboLabel, const char* bonusLabel,
                              int32_t& type, int32_t& bonus) {
            const char* preview = (type >= 0 && type <= 14) ? skillTypeNames[type] : "(None)";
            if (ImGui::BeginCombo(comboLabel, preview)) {
                bool noneSelected = (type < 0);
                if (ImGui::Selectable("(None)", noneSelected)) {
                    document_->undoStack->execute(
                        makeSetFieldCommand(&type, static_cast<int32_t>(-1), std::string("Change ") + comboLabel));
                    document_->dirty = true;
                }
                if (noneSelected) ImGui::SetItemDefaultFocus();
                for (int s = 0; s <= 14; ++s) {
                    bool sel = (type == s);
                    if (ImGui::Selectable(skillTypeNames[s], sel)) {
                        document_->undoStack->execute(
                            makeSetFieldCommand(&type, static_cast<int32_t>(s), std::string("Change ") + comboLabel));
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::EndCombo();
            }
            int b = bonus;
            if (ImGui::DragInt(bonusLabel, &b)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&bonus, static_cast<int32_t>(b), std::string("Change ") + bonusLabel));
                document_->dirty = true;
            }
        };

        skillCombo("Skill Type 1", "Skill Bonus 1", slot.skillType1, slot.skillBonus1);
        skillCombo("Skill Type 2", "Skill Bonus 2", slot.skillType2, slot.skillBonus2);
    }

    // Resist type combos with bonus inputs.
    {
        auto resistCombo = [&](const char* comboLabel, const char* bonusLabel,
                               int32_t& type, int32_t& bonus) {
            const char* preview = (type >= 0 && type <= 9) ? resistTypeNames[type] : "(None)";
            if (ImGui::BeginCombo(comboLabel, preview)) {
                bool noneSelected = (type < 0);
                if (ImGui::Selectable("(None)", noneSelected)) {
                    document_->undoStack->execute(
                        makeSetFieldCommand(&type, static_cast<int32_t>(-1), std::string("Change ") + comboLabel));
                    document_->dirty = true;
                }
                if (noneSelected) ImGui::SetItemDefaultFocus();
                for (int r = 0; r <= 9; ++r) {
                    bool sel = (type == r);
                    if (ImGui::Selectable(resistTypeNames[r], sel)) {
                        document_->undoStack->execute(
                            makeSetFieldCommand(&type, static_cast<int32_t>(r), std::string("Change ") + comboLabel));
                        document_->dirty = true;
                    }
                    if (sel) ImGui::SetItemDefaultFocus();
                }
                ImGui::EndCombo();
            }
            int b = bonus;
            if (ImGui::DragInt(bonusLabel, &b)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&bonus, static_cast<int32_t>(b), std::string("Change ") + bonusLabel));
                document_->dirty = true;
            }
        };

        resistCombo("Resist Type 1", "Resist Bonus 1", slot.resistType1, slot.resistBonus1);
        resistCombo("Resist Type 2", "Resist Bonus 2", slot.resistType2, slot.resistBonus2);
    }

    // Attribute inputs with resolved name suffix.
    {
        int att1 = slot.attribute1Index;
        const char* att1Name = nameDictionary_.itemAttName(att1);
        char att1Label[64];
        if (att1Name) {
            snprintf(att1Label, sizeof(att1Label), "Attribute 1 (%s)", att1Name);
        } else {
            snprintf(att1Label, sizeof(att1Label), "Attribute 1");
        }
        if (ImGui::DragInt(att1Label, &att1)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.attribute1Index, static_cast<int32_t>(att1), "Change attribute 1"));
            document_->dirty = true;
        }

        int att2 = slot.attribute2Index;
        const char* att2Name = nameDictionary_.itemAttName(att2);
        char att2Label[64];
        if (att2Name) {
            snprintf(att2Label, sizeof(att2Label), "Attribute 2 (%s)", att2Name);
        } else {
            snprintf(att2Label, sizeof(att2Label), "Attribute 2");
        }
        if (ImGui::DragInt(att2Label, &att2)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.attribute2Index, static_cast<int32_t>(att2), "Change attribute 2"));
            document_->dirty = true;
        }
    }

    // Rarely-changed fields in a collapsed section.
    if (ImGui::TreeNode("Other Fields")) {
        int autoId = static_cast<int>(slot.autoId);
        if (ImGui::DragInt("Auto ID", &autoId)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.autoId, static_cast<uint32_t>(autoId), "Change auto ID"));
            document_->dirty = true;
        }

        int power = slot.itemPower;
        if (ImGui::DragInt("Item Power", &power)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.itemPower, static_cast<int16_t>(power), "Change item power"));
            document_->dirty = true;
        }

        int equipped = slot.equippedFlag;
        if (ImGui::DragInt("Equipped Flag", &equipped, 1, 0, 65535)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.equippedFlag, static_cast<uint16_t>(std::max(0, equipped)), "Change equipped flag"));
            document_->dirty = true;
        }

        int reserved = slot.reserved;
        if (ImGui::DragInt("Reserved", &reserved, 1, 0, 65535)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.reserved, static_cast<uint16_t>(std::max(0, reserved)), "Change reserved"));
            document_->dirty = true;
        }

        int cat = slot.slotCategory;
        if (ImGui::DragInt("Slot Category", &cat)) {
            document_->undoStack->execute(
                makeSetFieldCommand(&slot.slotCategory, static_cast<int32_t>(cat), "Change slot category"));
            document_->dirty = true;
        }

        ImGui::TreePop();
    }

    ImGui::TreePop();
}

void SaveEditorTab::drawRosterSection() {
    const auto& units = document_->saveData->units();
    auto& roster = document_->saveData->roster();

    // Player units — the world map barracks view.
    ImGui::Text("Player Units");
    ImGui::Separator();

    if (ImGui::BeginTable("PlayerUnitsTable", 5,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg)) {
        ImGui::TableSetupColumn("#", ImGuiTableColumnFlags_WidthFixed, 40.0f);
        ImGui::TableSetupColumn("Name", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableSetupColumn("Level", ImGuiTableColumnFlags_WidthFixed, 50.0f);
        ImGui::TableSetupColumn("CharID", ImGuiTableColumnFlags_WidthFixed, 60.0f);
        ImGui::TableSetupColumn("Hero", ImGuiTableColumnFlags_WidthFixed, 40.0f);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < units.size(); ++i) {
            const auto& unit = units[i];
            if (unit.ucd != 0) continue;

            ImGui::TableNextRow();

            ImGui::TableSetColumnIndex(0);
            ImGui::Text("%zu", i);

            ImGui::TableSetColumnIndex(1);
            const char* name = resolveUnitName(unit, nameDictionary_);
            if (name) {
                ImGui::Text("%s", name);
            } else {
                ImGui::Text("Job %u", unit.jobType);
            }

            ImGui::TableSetColumnIndex(2);
            ImGui::Text("%u", unit.skillLevel);

            ImGui::TableSetColumnIndex(3);
            ImGui::Text("%d", unit.charId);

            ImGui::TableSetColumnIndex(4);
            ImGui::Text("%s", unit.isHero == 0 ? "Y" : "");
        }

        ImGui::EndTable();
    }

    // Raw roster records.
    ImGui::Spacing();
    ImGui::Text("Roster Records (%zu entries)", roster.size());
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
                document_->undoStack->execute(
                    makeSetFieldCommand(&roster[i].byte60, static_cast<uint8_t>(std::clamp(b60, 0, 255)), "Change roster byte60"));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(2);
            int b61 = roster[i].byte61;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b61", &b61, 1, 0, 255)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&roster[i].byte61, static_cast<uint8_t>(std::clamp(b61, 0, 255)), "Change roster byte61"));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(3);
            int b62 = roster[i].byte62;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b62", &b62, 1, 0, 255)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&roster[i].byte62, static_cast<uint8_t>(std::clamp(b62, 0, 255)), "Change roster byte62"));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(4);
            int b63 = roster[i].byte63;
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##b63", &b63, 1, 0, 255)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&roster[i].byte63, static_cast<uint8_t>(std::clamp(b63, 0, 255)), "Change roster byte63"));
                document_->dirty = true;
            }

            ImGui::TableSetColumnIndex(5);
            int val = static_cast<int>(roster[i].value64);
            ImGui::SetNextItemWidth(-1);
            if (ImGui::DragInt("##val", &val, 1, 0, 0)) {
                document_->undoStack->execute(
                    makeSetFieldCommand(&roster[i].value64, static_cast<uint32_t>(val), "Change roster value64"));
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
        document_->undoStack->execute(
            makeSetFieldCommand(&save->currentMissionIndexRef(), static_cast<int32_t>(missionIdx), "Change current mission index"));
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
            {
                int32_t prev = completion[i];
                if (ImGui::DragInt("##mc", &completion[i], 1, 0, 0)) {
                    int32_t newVal = completion[i];
                    completion[i] = prev;
                    document_->undoStack->execute(
                        makeSetFieldCommand(&completion[i], newVal, "Change mission completion"));
                    document_->dirty = true;
                }
            }
            ImGui::PopID();

            if (i + 1 < kSaveMissionSlots) {
                ImGui::TableSetColumnIndex(2);
                ImGui::Text("[%zu]", i + 1);

                ImGui::TableSetColumnIndex(3);
                ImGui::PushID(static_cast<int>(i + 1));
                ImGui::SetNextItemWidth(-1);
                {
                    int32_t prev = completion[i + 1];
                    if (ImGui::DragInt("##mc", &completion[i + 1], 1, 0, 0)) {
                        int32_t newVal = completion[i + 1];
                        completion[i + 1] = prev;
                        document_->undoStack->execute(
                            makeSetFieldCommand(&completion[i + 1], newVal, "Change mission completion"));
                        document_->dirty = true;
                    }
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
                    document_->undoStack->execute(
                        makeSetFieldCommand(&secondArr[i], static_cast<uint32_t>(val), std::string("Change ") + label));
                    document_->dirty = true;
                }
                ImGui::PopID();
            }
        }
    }
}

} // namespace kuf
