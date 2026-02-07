#pragma once

#include "ui/tabs/editor_tab.h"
#include "formats/stg_format.h"

#include <memory>

namespace kuf {

class StgEditorTab : public EditorTab {
public:
    explicit StgEditorTab(std::shared_ptr<OpenDocument> doc);

    void drawContent() override;

    void selectUnit(size_t index);
    int selectedUnit() const { return selectedUnit_; }

private:
    enum class Section {
        Header,
        Units
    };

    void drawSidebar();
    void drawHeaderSection();
    void drawUnitList();
    void drawUnitDetails(size_t index);
    void drawOfficerSection(const char* label, OfficerData& officer, bool active);

    Section currentSection_ = Section::Units;
    int selectedUnit_ = -1;
};

} // namespace kuf
