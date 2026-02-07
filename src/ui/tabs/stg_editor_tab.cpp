#include "ui/tabs/stg_editor_tab.h"

#include <imgui.h>

#include <algorithm>
#include <cstring>

namespace kuf {

namespace {

const char* ucdNames[] = {"Player", "Enemy", "Ally", "Neutral"};

const char* directionNames[] = {
    "East", "NorthEast", "North", "NorthWest",
    "West", "SouthWest", "South", "SouthEast"
};

struct JobTypeEntry {
    JobType type;
    const char* name;
};

const JobTypeEntry jobTypes[] = {
    {JobType::HumanArcher,          "Human Archer"},
    {JobType::HumanLongbows,        "Human Longbows"},
    {JobType::HumanInfantry,        "Human Infantry"},
    {JobType::HumanSpearman,        "Human Spearman"},
    {JobType::HumanHeavyInfantry,   "Human Heavy Infantry"},
    {JobType::HumanKnight,          "Human Knight"},
    {JobType::HumanPaladin,         "Human Paladin"},
    {JobType::HumanCavalry,         "Human Cavalry"},
    {JobType::HumanHeavyCavalry,    "Human Heavy Cavalry"},
    {JobType::HumanStormRiders,     "Human Storm Riders"},
    {JobType::HumanSapper,          "Human Sapper"},
    {JobType::HumanPyroTechnician,  "Human Pyro Technician"},
    {JobType::HumanBomberWing,      "Human Bomber Wing"},
    {JobType::HumanMortar,          "Human Mortar"},
    {JobType::HumanBallista,        "Human Ballista"},
    {JobType::HumanHarpoon,         "Human Harpoon"},
    {JobType::HumanCatapult,        "Human Catapult"},
    {JobType::HumanBattaloon,       "Human Battaloon"},
    {JobType::DarkElfArcher,        "Dark Elf Archer"},
    {JobType::DarkElfCavalryArcher, "Dark Elf Cavalry Archer"},
    {JobType::DarkElfFighter,       "Dark Elf Fighter"},
    {JobType::DarkElfKnight,        "Dark Elf Knight"},
    {JobType::DarkElfLightCavalry,  "Dark Elf Light Cavalry"},
    {JobType::DarkOrcInfantry,      "Dark Orc Infantry"},
    {JobType::DarkOrcRider,         "Dark Orc Rider"},
    {JobType::DarkOrcHeavyRiders,   "Dark Orc Heavy Riders"},
    {JobType::DarkOrcAxeMan,        "Dark Orc Axe Man"},
    {JobType::DarkOrcHeavyInfantry, "Dark Orc Heavy Infantry"},
    {JobType::DarkOrcSapper,        "Dark Orc Sapper"},
    {JobType::Scorpion,             "Scorpion"},
    {JobType::SwampMammoth,         "Swamp Mammoth"},
    {JobType::Dirigible,            "Dirigible"},
    {JobType::BlackWyvern,          "Black Wyvern"},
    {JobType::Ghoul,                "Ghoul"},
    {JobType::BoneDragon,           "Bone Dragon"},
    {JobType::Wall,                 "Wall"},
    {JobType::Scout,                "Scout"},
    {JobType::SelfDestruction,      "Self-Destruction"},
    {JobType::EncablosaMelee,       "Encablosa Melee"},
    {JobType::EncablosaFlying,      "Encablosa Flying"},
    {JobType::EncablosaRanged,      "Encablosa Ranged"},
    {JobType::ElfWall,              "Elf Wall"},
    {JobType::EncablosaLarge,       "Encablosa Large"},
};

const char* jobTypeName(JobType jt) {
    uint8_t idx = static_cast<uint8_t>(jt);
    if (idx < std::size(jobTypes)) {
        return jobTypes[idx].name;
    }
    return "Unknown";
}

ImVec4 ucdColor(UCD ucd) {
    switch (ucd) {
        case UCD::Player:  return ImVec4(0.2f, 0.8f, 0.2f, 1.0f);
        case UCD::Enemy:   return ImVec4(0.9f, 0.2f, 0.2f, 1.0f);
        case UCD::Ally:    return ImVec4(0.2f, 0.5f, 0.9f, 1.0f);
        case UCD::Neutral: return ImVec4(0.7f, 0.7f, 0.7f, 1.0f);
    }
    return ImVec4(1.0f, 1.0f, 1.0f, 1.0f);
}

void drawJobTypeCombo(const char* label, JobType& current) {
    int currentIdx = static_cast<int>(current);
    if (ImGui::BeginCombo(label, jobTypeName(current))) {
        for (size_t i = 0; i < std::size(jobTypes); ++i) {
            bool selected = (static_cast<int>(i) == currentIdx);
            if (ImGui::Selectable(jobTypes[i].name, selected)) {
                current = jobTypes[i].type;
            }
            if (selected) {
                ImGui::SetItemDefaultFocus();
            }
        }
        ImGui::EndCombo();
    }
}

} // namespace

StgEditorTab::StgEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {}

void StgEditorTab::selectUnit(size_t index) {
    if (document_ && document_->stgData &&
        index < document_->stgData->unitCount()) {
        selectedUnit_ = static_cast<int>(index);
        currentSection_ = Section::Units;
    }
}

void StgEditorTab::drawContent() {
    if (!document_ || !document_->stgData) {
        ImGui::TextDisabled("No STG data loaded");
        return;
    }

    float totalHeight = ImGui::GetContentRegionAvail().y;

    // Sidebar.
    ImGui::BeginChild("StgSidebar", ImVec2(120, totalHeight), ImGuiChildFlags_Borders);
    drawSidebar();
    ImGui::EndChild();

    ImGui::SameLine();

    if (currentSection_ == Section::Header) {
        ImGui::BeginChild("StgHeaderContent", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        drawHeaderSection();
        ImGui::EndChild();
    } else {
        // Unit list + details.
        ImGui::BeginChild("StgUnitList", ImVec2(230, totalHeight), ImGuiChildFlags_Borders);
        drawUnitList();
        ImGui::EndChild();

        ImGui::SameLine();

        ImGui::BeginChild("StgUnitDetails", ImVec2(0, totalHeight), ImGuiChildFlags_Borders);
        if (selectedUnit_ >= 0 &&
            selectedUnit_ < static_cast<int>(document_->stgData->unitCount())) {
            drawUnitDetails(selectedUnit_);
        } else {
            ImGui::TextDisabled("Select a unit to edit");
        }
        ImGui::EndChild();
    }
}

void StgEditorTab::drawSidebar() {
    ImGui::Text("Sections");
    ImGui::Separator();

    if (ImGui::Selectable("Header", currentSection_ == Section::Header)) {
        currentSection_ = Section::Header;
    }
    if (ImGui::Selectable("Units", currentSection_ == Section::Units)) {
        currentSection_ = Section::Units;
    }
}

void StgEditorTab::drawHeaderSection() {
    auto& hdr = document_->stgData->header();

    ImGui::Text("Mission Header");
    ImGui::Separator();

    int missionId = static_cast<int>(hdr.missionId);
    if (ImGui::InputInt("Mission ID", &missionId)) {
        hdr.missionId = static_cast<uint32_t>(std::max(0, missionId));
        document_->dirty = true;
    }

    char buf[64];

    auto stringInput = [&](const char* label, std::string& str) {
        std::memset(buf, 0, sizeof(buf));
        std::strncpy(buf, str.c_str(), sizeof(buf) - 1);
        if (ImGui::InputText(label, buf, sizeof(buf))) {
            str = buf;
            document_->dirty = true;
        }
    };

    if (ImGui::CollapsingHeader("File References", ImGuiTreeNodeFlags_DefaultOpen)) {
        stringInput("Map File", hdr.mapFile);
        stringInput("Bitmap File", hdr.bitmapFile);
        stringInput("Default Camera", hdr.defaultCameraFile);
        stringInput("User Camera", hdr.userCameraFile);
        stringInput("Settings File", hdr.settingsFile);
        stringInput("Sky/Cloud Effects", hdr.skyCloudEffects);
        stringInput("AI Script", hdr.aiScriptFile);
        stringInput("Cubemap Texture", hdr.cubemapTexture);
    }

    ImGui::Separator();
    ImGui::Text("Unit Count: %u", hdr.unitCount);
}

void StgEditorTab::drawUnitList() {
    const auto& units = document_->stgData->units();

    for (size_t i = 0; i < units.size(); ++i) {
        const auto& unit = units[i];
        bool selected = (selectedUnit_ == static_cast<int>(i));

        // Color-code by UCD.
        ImGui::PushStyleColor(ImGuiCol_Text, ucdColor(unit.ucd));

        char label[64];
        if (unit.unitName.empty()) {
            snprintf(label, sizeof(label), "[%zu] %s", i, jobTypeName(unit.leaderJobType));
        } else {
            snprintf(label, sizeof(label), "[%zu] %s", i, unit.unitName.c_str());
        }

        if (ImGui::Selectable(label, selected)) {
            selectedUnit_ = static_cast<int>(i);
        }

        ImGui::PopStyleColor();

        if (ImGui::IsItemHovered()) {
            ImGui::SetTooltip("ID: %u | %s | %s | Lv%d",
                unit.uniqueId,
                ucdNames[static_cast<int>(unit.ucd)],
                jobTypeName(unit.leaderJobType),
                unit.leaderLevel);
        }
    }
}

void StgEditorTab::drawUnitDetails(size_t index) {
    auto& unit = document_->stgData->units()[index];

    ImGui::Text("[%zu] %s", index, unit.unitName.empty() ? "(unnamed)" : unit.unitName.c_str());
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Core", ImGuiTreeNodeFlags_DefaultOpen)) {
        char nameBuf[32];
        std::memset(nameBuf, 0, sizeof(nameBuf));
        std::strncpy(nameBuf, unit.unitName.c_str(), sizeof(nameBuf) - 1);
        if (ImGui::InputText("Name", nameBuf, sizeof(nameBuf))) {
            unit.unitName = nameBuf;
            document_->dirty = true;
        }

        int uid = static_cast<int>(unit.uniqueId);
        if (ImGui::InputInt("Unique ID", &uid)) {
            unit.uniqueId = static_cast<uint32_t>(std::max(0, uid));
            document_->dirty = true;
        }

        int ucdIdx = static_cast<int>(unit.ucd);
        if (ImGui::Combo("UCD", &ucdIdx, ucdNames, IM_ARRAYSIZE(ucdNames))) {
            unit.ucd = static_cast<UCD>(ucdIdx);
            document_->dirty = true;
        }

        bool hero = unit.isHero != 0;
        if (ImGui::Checkbox("Is Hero", &hero)) {
            unit.isHero = hero ? 1 : 0;
            document_->dirty = true;
        }

        ImGui::SameLine();
        bool enabled = unit.isEnabled != 0;
        if (ImGui::Checkbox("Is Enabled", &enabled)) {
            unit.isEnabled = enabled ? 1 : 0;
            document_->dirty = true;
        }

        if (ImGui::DragFloat("Leader HP Override", &unit.leaderHpOverride, 1.0f, -1.0f, 100000.0f, "%.1f")) {
            document_->dirty = true;
        }
        if (ImGui::IsItemHovered()) ImGui::SetTooltip("-1.0 = use default");

        if (ImGui::DragFloat("Unit HP Override", &unit.unitHpOverride, 1.0f, -1.0f, 100000.0f, "%.1f")) {
            document_->dirty = true;
        }
        if (ImGui::IsItemHovered()) ImGui::SetTooltip("-1.0 = use default");
    }

    if (ImGui::CollapsingHeader("Position", ImGuiTreeNodeFlags_DefaultOpen)) {
        if (ImGui::DragFloat("X", &unit.positionX, 10.0f, -100000.0f, 100000.0f, "%.1f")) {
            document_->dirty = true;
        }
        if (ImGui::DragFloat("Y", &unit.positionY, 10.0f, -100000.0f, 100000.0f, "%.1f")) {
            document_->dirty = true;
        }

        int dirIdx = static_cast<int>(unit.direction);
        if (ImGui::Combo("Direction", &dirIdx, directionNames, IM_ARRAYSIZE(directionNames))) {
            unit.direction = static_cast<Direction>(dirIdx);
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Leader", ImGuiTreeNodeFlags_DefaultOpen)) {
        drawJobTypeCombo("Job Type", unit.leaderJobType);

        int modelId = unit.leaderModelId;
        if (ImGui::InputInt("Model ID", &modelId)) {
            unit.leaderModelId = static_cast<uint8_t>(std::clamp(modelId, 0, 255));
            document_->dirty = true;
        }

        int wmId = unit.leaderWorldmapId;
        if (ImGui::InputInt("Worldmap ID", &wmId)) {
            unit.leaderWorldmapId = static_cast<uint8_t>(std::clamp(wmId, 0, 255));
            document_->dirty = true;
        }
        if (ImGui::IsItemHovered()) ImGui::SetTooltip("0xFF = safe default (no worldmap state)");

        int level = unit.leaderLevel;
        if (ImGui::InputInt("Level", &level)) {
            unit.leaderLevel = static_cast<uint8_t>(std::clamp(level, 1, 99));
            document_->dirty = true;
        }
    }

    if (ImGui::CollapsingHeader("Skills")) {
        for (int i = 0; i < 4; ++i) {
            ImGui::PushID(i);
            char skillLabel[32];
            snprintf(skillLabel, sizeof(skillLabel), "Skill %d", i + 1);

            int skillId = unit.leaderSkills[i].skillId;
            int skillLv = unit.leaderSkills[i].level;

            ImGui::Text("%s:", skillLabel);
            ImGui::SameLine();
            ImGui::SetNextItemWidth(80);
            if (ImGui::InputInt("##id", &skillId)) {
                unit.leaderSkills[i].skillId = static_cast<uint8_t>(std::clamp(skillId, 0, 255));
                document_->dirty = true;
            }
            ImGui::SameLine();
            ImGui::Text("Lv:");
            ImGui::SameLine();
            ImGui::SetNextItemWidth(60);
            if (ImGui::InputInt("##lv", &skillLv)) {
                unit.leaderSkills[i].level = static_cast<uint8_t>(std::clamp(skillLv, 0, 255));
                document_->dirty = true;
            }
            ImGui::PopID();
        }
    }

    if (ImGui::CollapsingHeader("Abilities")) {
        for (int i = 0; i < 23; ++i) {
            ImGui::PushID(i + 100);
            char abilLabel[32];
            snprintf(abilLabel, sizeof(abilLabel), "Slot %d", i);

            int val = unit.leaderAbilities[i];
            if (val == -1) {
                ImGui::TextDisabled("%s: Empty", abilLabel);
                ImGui::SameLine();
                if (ImGui::SmallButton("Set")) {
                    unit.leaderAbilities[i] = 0;
                    document_->dirty = true;
                }
            } else {
                ImGui::SetNextItemWidth(120);
                if (ImGui::InputInt(abilLabel, &val)) {
                    unit.leaderAbilities[i] = val;
                    document_->dirty = true;
                }
                ImGui::SameLine();
                if (ImGui::SmallButton("Clear")) {
                    unit.leaderAbilities[i] = -1;
                    document_->dirty = true;
                }
            }
            ImGui::PopID();
        }
    }

    if (ImGui::CollapsingHeader("Officers")) {
        int count = static_cast<int>(unit.officerCount);
        if (ImGui::SliderInt("Officer Count", &count, 0, 2)) {
            unit.officerCount = static_cast<uint32_t>(count);
            document_->dirty = true;
        }

        if (unit.officerCount >= 1) {
            drawOfficerSection("Officer 1", unit.officer1, true);
        }
        if (unit.officerCount >= 2) {
            drawOfficerSection("Officer 2", unit.officer2, true);
        }
    }

    if (ImGui::CollapsingHeader("Unit Configuration")) {
        int troopIdx = static_cast<int>(unit.troopInfoIndex);
        if (ImGui::InputInt("TroopInfo Index", &troopIdx)) {
            unit.troopInfoIndex = static_cast<uint32_t>(std::max(0, troopIdx));
            document_->dirty = true;
        }

        int formation = static_cast<int>(unit.formationType);
        if (ImGui::InputInt("Formation", &formation)) {
            unit.formationType = static_cast<uint32_t>(std::max(0, formation));
            document_->dirty = true;
        }

        int gx = static_cast<int>(unit.gridX);
        int gy = static_cast<int>(unit.gridY);
        if (ImGui::InputInt("Grid X", &gx)) {
            unit.gridX = static_cast<uint32_t>(std::max(1, gx));
            document_->dirty = true;
        }
        if (ImGui::InputInt("Grid Y", &gy)) {
            unit.gridY = static_cast<uint32_t>(std::max(1, gy));
            document_->dirty = true;
        }
        ImGui::Text("Total Units: %u", unit.gridX * unit.gridY);
    }

    if (ImGui::CollapsingHeader("Stat Overrides")) {
        ImGui::TextDisabled("Values of -1.0 use TroopInfo defaults");
        ImGui::Separator();

        for (int i = 0; i < 22; ++i) {
            ImGui::PushID(i + 200);
            char label[32];
            snprintf(label, sizeof(label), "Override %d", i);

            if (ImGui::DragFloat(label, &unit.statOverrides[i], 1.0f, -1.0f, 100000.0f, "%.1f")) {
                document_->dirty = true;
            }
            ImGui::PopID();
        }
    }
}

void StgEditorTab::drawOfficerSection(const char* label, OfficerData& officer, bool active) {
    if (!active) return;

    ImGui::PushID(label);
    if (ImGui::TreeNode(label)) {
        drawJobTypeCombo("Job Type", officer.jobType);

        int modelId = officer.modelId;
        if (ImGui::InputInt("Model ID", &modelId)) {
            officer.modelId = static_cast<uint8_t>(std::clamp(modelId, 0, 255));
            document_->dirty = true;
        }

        int wmId = officer.worldmapId;
        if (ImGui::InputInt("Worldmap ID", &wmId)) {
            officer.worldmapId = static_cast<uint8_t>(std::clamp(wmId, 0, 255));
            document_->dirty = true;
        }

        int level = officer.level;
        if (ImGui::InputInt("Level", &level)) {
            officer.level = static_cast<uint8_t>(std::clamp(level, 1, 99));
            document_->dirty = true;
        }

        if (ImGui::TreeNode("Skills")) {
            for (int i = 0; i < 4; ++i) {
                ImGui::PushID(i);
                int sid = officer.skills[i].skillId;
                int slv = officer.skills[i].level;
                ImGui::Text("Skill %d:", i + 1);
                ImGui::SameLine();
                ImGui::SetNextItemWidth(80);
                if (ImGui::InputInt("##id", &sid)) {
                    officer.skills[i].skillId = static_cast<uint8_t>(std::clamp(sid, 0, 255));
                    document_->dirty = true;
                }
                ImGui::SameLine();
                ImGui::Text("Lv:");
                ImGui::SameLine();
                ImGui::SetNextItemWidth(60);
                if (ImGui::InputInt("##lv", &slv)) {
                    officer.skills[i].level = static_cast<uint8_t>(std::clamp(slv, 0, 255));
                    document_->dirty = true;
                }
                ImGui::PopID();
            }
            ImGui::TreePop();
        }

        ImGui::TreePop();
    }
    ImGui::PopID();
}

} // namespace kuf
