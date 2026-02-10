#pragma once

#include "ui/tabs/editor_tab.h"
#include "formats/sox_skill_info.h"

#include <memory>

namespace kuf {

class SkillEditorTab : public EditorTab {
public:
    explicit SkillEditorTab(std::shared_ptr<OpenDocument> doc);

    void drawContent() override;

    void selectSkill(size_t index);
    int selectedSkill() const { return selectedSkill_; }

private:
    void drawSkillList();
    void drawSkillDetails(size_t index);

    int selectedSkill_ = -1;

    static constexpr const char* SKILL_NAMES[] = {
        "Melee", "Ranged", "Frontal", "Riding", "Teamwork",
        "Scouting", "Gunpowder", "Beast Mastery", "Fire", "Lightning",
        "Ice", "Holy", "Earth", "Curse", "Any Elemental"
    };
};

} // namespace kuf
