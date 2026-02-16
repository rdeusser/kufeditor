#pragma once

#include "ui/tabs/editor_tab.h"
#include "formats/save_format.h"

#include <memory>

namespace kuf {

class SaveEditorTab : public EditorTab {
public:
    explicit SaveEditorTab(std::shared_ptr<OpenDocument> doc);

    void drawContent() override;

private:
    enum class Section {
        Summary,
        Units,
        Roster,
        Missions
    };

    void drawSidebar();
    void drawSummarySection();
    void drawUnitList();
    void drawUnitDetails(size_t index);
    void drawAbilitySets(SaveUnit& unit);
    void drawRosterSection();
    void drawMissionsSection();

    Section currentSection_ = Section::Summary;
    int selectedUnit_ = -1;
};

} // namespace kuf
