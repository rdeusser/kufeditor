#include "ui/tabs/troop_editor_tab.h"

#include <cfloat>
#include <imgui.h>

namespace kuf {

TroopEditorTab::TroopEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {}

void TroopEditorTab::selectTroop(size_t index) {
    if (document_ && document_->binaryData &&
        index < document_->binaryData->troops().size()) {
        selectedTroop_ = static_cast<int>(index);
    }
}

void TroopEditorTab::drawContent() {
    if (!document_ || !document_->binaryData) {
        ImGui::TextDisabled("No troop data loaded");
        return;
    }

    float listHeight = ImGui::GetContentRegionAvail().y;
    ImGui::BeginChild("TroopList", ImVec2(250, listHeight), ImGuiChildFlags_Borders);
    drawTroopTable();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("TroopDetails", ImVec2(0, listHeight), ImGuiChildFlags_Borders);
    if (selectedTroop_ >= 0 &&
        selectedTroop_ < static_cast<int>(document_->binaryData->troops().size())) {
        drawTroopDetails(selectedTroop_);
    } else {
        ImGui::TextDisabled("Select a troop to edit");
    }
    ImGui::EndChild();
}

void TroopEditorTab::drawTroopTable() {
    const auto& troops = document_->binaryData->troops();
    for (size_t i = 0; i < troops.size(); ++i) {
        const char* name = (i < std::size(TROOP_NAMES)) ? TROOP_NAMES[i] : "Unknown";
        bool selected = (selectedTroop_ == static_cast<int>(i));

        if (ImGui::Selectable(name, selected)) {
            selectedTroop_ = static_cast<int>(i);
        }
    }
}

void TroopEditorTab::drawTroopDetails(size_t index) {
    auto& troop = document_->binaryData->troops()[index];
    const char* name = (index < std::size(TROOP_NAMES)) ? TROOP_NAMES[index] : "Unknown";

    ImGui::Text("%s", name);
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Movement", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Move Speed", &troop.moveSpeed, 1.0f, 0.0f, 10000.0f, "%.0f");
        ImGui::DragFloat("Rotate Rate", &troop.rotateRate, 1.0f, 0.0f, 1000.0f, "%.0f");
        ImGui::DragFloat("Acceleration", &troop.moveAcceleration, 1.0f, 0.0f, 1000.0f, "%.0f");
        ImGui::DragFloat("Deceleration", &troop.moveDeceleration, 1.0f, 0.0f, 1000.0f, "%.0f");
    }

    if (ImGui::CollapsingHeader("Combat", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Sight Range", &troop.sightRange, 10.0f, 0.0f, 50000.0f, "%.0f");
        ImGui::DragFloat("Attack Range Max", &troop.attackRangeMax, 10.0f, 0.0f, 50000.0f, "%.0f");
        ImGui::DragFloat("Attack Range Min", &troop.attackRangeMin, 10.0f, 0.0f, 50000.0f, "%.0f");
        ImGui::DragFloat("Indirect Attack", &troop.indirectAttack, 1.0f, 0.0f, 1000.0f, "%.0f");
        ImGui::DragFloat("Direct Attack", &troop.directAttack, 1.0f, 0.0f, 1000.0f, "%.0f");
        ImGui::DragFloat("Defense", &troop.defense, 1.0f, 0.0f, 1000.0f, "%.0f");
    }

    if (ImGui::CollapsingHeader("Resistances", ImGuiTreeNodeFlags_DefaultOpen)) {
        // File stores: 0=immune, 100=normal, 200=very vulnerable.
        // Game UI shows: 100=immune, 0=normal, -100=very vulnerable.
        // Formula: display = 100 - file_value
        auto resistInput = [](const char* label, float* value) {
            int fileVal = static_cast<int>(*value);
            int displayVal = 100 - fileVal;
            if (ImGui::DragInt(label, &displayVal, 1, -200, 100, "%+d")) {
                *value = static_cast<float>(100 - displayVal);
            }
        };

        resistInput("Melee", &troop.resistMelee);
        resistInput("Ranged", &troop.resistRanged);
        resistInput("Frontal", &troop.resistFrontal);
        resistInput("Explosion", &troop.resistExplosion);
        resistInput("Fire", &troop.resistFire);
        resistInput("Ice", &troop.resistIce);
        resistInput("Lightning", &troop.resistLightning);
        resistInput("Holy", &troop.resistHoly);
        resistInput("Curse", &troop.resistCurse);
        resistInput("Earth", &troop.resistEarth);
    }

    if (ImGui::CollapsingHeader("Unit Configuration", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Default HP", &troop.defaultUnitHp, 1.0f, 1.0f, 10000.0f, "%.0f");
        ImGui::DragInt("Units X", &troop.defaultUnitNumX, 1, 1, 20);
        ImGui::DragInt("Units Y", &troop.defaultUnitNumY, 1, 1, 20);
        ImGui::Text("Total Units: %d", troop.defaultUnitNumX * troop.defaultUnitNumY);
    }
}

} // namespace kuf
