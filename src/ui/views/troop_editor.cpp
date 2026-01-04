#include "ui/views/troop_editor.h"

#include <imgui.h>

namespace kuf {

TroopEditorView::TroopEditorView() : View("Troop Editor") {}

void TroopEditorView::setData(std::shared_ptr<SoxBinary> data) {
    data_ = std::move(data);
    selectedTroop_ = -1;
}

void TroopEditorView::drawContent() {
    if (!data_) {
        ImGui::TextDisabled("No file loaded. Use File > Open to load TroopInfo.sox");
        return;
    }

    ImGui::BeginChild("TroopList", ImVec2(250, 0), true);
    drawTroopTable();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("TroopDetails", ImVec2(0, 0), true);
    if (selectedTroop_ >= 0 && selectedTroop_ < static_cast<int>(data_->troops().size())) {
        drawTroopDetails(selectedTroop_);
    } else {
        ImGui::TextDisabled("Select a troop to edit");
    }
    ImGui::EndChild();
}

void TroopEditorView::drawTroopTable() {
    for (size_t i = 0; i < data_->troops().size(); ++i) {
        const char* name = (i < std::size(TROOP_NAMES)) ? TROOP_NAMES[i] : "Unknown";
        bool selected = (selectedTroop_ == static_cast<int>(i));

        if (ImGui::Selectable(name, selected)) {
            selectedTroop_ = static_cast<int>(i);
        }
    }
}

void TroopEditorView::drawTroopDetails(size_t index) {
    auto& troop = data_->troops()[index];
    const char* name = (index < std::size(TROOP_NAMES)) ? TROOP_NAMES[index] : "Unknown";

    ImGui::Text("%s", name);
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Movement", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Move Speed", &troop.moveSpeed, 1.0f, 0.0f, 10000.0f);
        ImGui::DragFloat("Rotate Rate", &troop.rotateRate, 0.01f, 0.0f, 10.0f);
        ImGui::DragFloat("Acceleration", &troop.moveAcceleration, 1.0f, 0.0f, 1000.0f);
        ImGui::DragFloat("Deceleration", &troop.moveDeceleration, 1.0f, 0.0f, 1000.0f);
    }

    if (ImGui::CollapsingHeader("Combat", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Sight Range", &troop.sightRange, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Attack Range Max", &troop.attackRangeMax, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Attack Range Min", &troop.attackRangeMin, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Direct Attack", &troop.directAttack, 0.1f, 0.0f, 100.0f);
        ImGui::DragFloat("Indirect Attack", &troop.indirectAttack, 0.1f, 0.0f, 100.0f);
        ImGui::DragFloat("Defense", &troop.defense, 0.1f, 0.0f, 100.0f);
    }

    if (ImGui::CollapsingHeader("Resistances", ImGuiTreeNodeFlags_DefaultOpen)) {
        auto resistSlider = [](const char* label, float* value) {
            ImGui::SliderFloat(label, value, 0.0f, 2.0f, "%.2f");
            ImGui::SameLine();
            int pct = static_cast<int>((1.0f - *value) * 100.0f);
            ImGui::Text("(%+d%%)", pct);
        };

        resistSlider("Melee", &troop.resistMelee);
        resistSlider("Ranged", &troop.resistRanged);
        resistSlider("Frontal", &troop.resistFrontal);
        resistSlider("Explosion", &troop.resistExplosion);
        resistSlider("Fire", &troop.resistFire);
        resistSlider("Ice", &troop.resistIce);
        resistSlider("Lightning", &troop.resistLightning);
        resistSlider("Holy", &troop.resistHoly);
        resistSlider("Curse", &troop.resistCurse);
        resistSlider("Poison", &troop.resistPoison);
    }

    if (ImGui::CollapsingHeader("Unit Configuration")) {
        ImGui::DragFloat("Default HP", &troop.defaultUnitHp, 1.0f, 1.0f, 10000.0f);
        ImGui::DragInt("Units X", &troop.defaultUnitNumX, 1, 1, 20);
        ImGui::DragInt("Units Y", &troop.defaultUnitNumY, 1, 1, 20);
        ImGui::Text("Total Units: %d", troop.defaultUnitNumX * troop.defaultUnitNumY);
    }
}

} // namespace kuf
