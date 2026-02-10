#include "ui/tabs/skill_editor_tab.h"

#include <algorithm>
#include <cstring>
#include <imgui.h>

#include "ui/imgui_helpers.h"

namespace kuf {

SkillEditorTab::SkillEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {}

void SkillEditorTab::selectSkill(size_t index) {
    if (document_ && document_->skillData &&
        index < document_->skillData->skills().size()) {
        selectedSkill_ = static_cast<int>(index);
    }
}

void SkillEditorTab::drawContent() {
    if (!document_ || !document_->skillData) {
        ImGui::TextDisabled("No skill data loaded");
        return;
    }

    float listHeight = ImGui::GetContentRegionAvail().y;
    ImGui::BeginChild("SkillList", ImVec2(250, listHeight), ImGuiChildFlags_Borders);
    drawSkillList();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("SkillDetails", ImVec2(0, listHeight), ImGuiChildFlags_Borders);
    if (selectedSkill_ >= 0 &&
        selectedSkill_ < static_cast<int>(document_->skillData->skills().size())) {
        drawSkillDetails(selectedSkill_);
    } else {
        ImGui::TextDisabled("Select a skill to edit");
    }
    ImGui::EndChild();
}

void SkillEditorTab::drawSkillList() {
    const auto& skills = document_->skillData->skills();
    for (size_t i = 0; i < skills.size(); ++i) {
        const char* name = (i < std::size(SKILL_NAMES)) ? SKILL_NAMES[i] : "Unknown";
        bool selected = (selectedSkill_ == static_cast<int>(i));

        if (ImGui::Selectable(name, selected)) {
            selectedSkill_ = static_cast<int>(i);
        }
    }
}

void SkillEditorTab::drawSkillDetails(size_t index) {
    auto& skill = document_->skillData->skills()[index];
    const char* name = (index < std::size(SKILL_NAMES)) ? SKILL_NAMES[index] : "Unknown";

    ImGui::Text("%s", name);
    ImGui::Separator();

    ImGui::PushItemWidth(-ImGui::CalcTextSize("Localization Key  ").x);

    int id = skill.id;
    if (ImGui::DragInt("Skill ID", &id)) {
        skill.id = id;
        document_->dirty = true;
    }

    char locBuf[256];
    std::strncpy(locBuf, skill.locKey.c_str(), sizeof(locBuf) - 1);
    locBuf[sizeof(locBuf) - 1] = '\0';
    if (InputTextCentered("Localization Key", locBuf, sizeof(locBuf))) {
        skill.locKey = locBuf;
        document_->dirty = true;
    }

    char iconBuf[256];
    std::strncpy(iconBuf, skill.iconPath.c_str(), sizeof(iconBuf) - 1);
    iconBuf[sizeof(iconBuf) - 1] = '\0';
    if (InputTextCentered("Icon Path", iconBuf, sizeof(iconBuf))) {
        skill.iconPath = iconBuf;
        document_->dirty = true;
    }

    int slotCount = static_cast<int>(skill.slotCount);
    if (ImGui::DragInt("Slot Count", &slotCount, 1, 1, 4)) {
        skill.slotCount = static_cast<uint32_t>(std::clamp(slotCount, 1, 4));
        document_->dirty = true;
    }

    int maxLevel = static_cast<int>(skill.maxLevel);
    if (ImGui::DragInt("Max Level", &maxLevel, 1, 1, 65535)) {
        skill.maxLevel = static_cast<uint32_t>(std::clamp(maxLevel, 1, 65535));
        document_->dirty = true;
    }

    ImGui::PopItemWidth();
}

} // namespace kuf
