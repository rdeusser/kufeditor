#pragma once

#include "ui/views/view.h"
#include "formats/sox_binary.h"

#include <memory>

namespace kuf {

class TroopEditorView : public View {
public:
    TroopEditorView();

    void drawContent() override;

    void setData(std::shared_ptr<SoxBinary> data);
    bool hasData() const { return data_ != nullptr; }

private:
    void drawTroopTable();
    void drawTroopDetails(size_t index);

    std::shared_ptr<SoxBinary> data_;
    int selectedTroop_ = -1;

    static constexpr const char* TROOP_NAMES[] = {
        "Archer", "Longbows", "Infantry", "Spearman", "Heavy Infantry",
        "Knight", "Paladin", "Cavalry", "Heavy Cavalry", "Storm Riders",
        "Sappers", "Pyro Techs", "Bomber Wings", "Mortar", "Ballista",
        "Harpoon", "Catapult", "Battaloon", "Dark Elves Archer",
        "Dark Elves Cavalry Archers", "Dark Elves Infantry", "Dark Elves Knights",
        "Dark Elves Cavalry", "Orc Infantry", "Orc Riders", "Orc Heavy Riders",
        "Orc Axe Man", "Orc Heavy Infantry", "Orc Sappers", "Orc Scorpion",
        "Orc Swamp Mammoth", "Orc Dirigible", "Orc Black Wyverns", "Orc Ghouls",
        "Orc Bone Dragon", "Wall Archers (Humans)", "Scouts", "Ghoul Selfdestruct",
        "Encablossa Monster (Melee)", "Encablossa Flying Monster",
        "Encablossa Monster (Ranged)", "Wall Archers (Elves)", "Encablossa Main"
    };
};

} // namespace kuf
