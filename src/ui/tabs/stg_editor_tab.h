#pragma once

#include "ui/tabs/editor_tab.h"
#include "core/name_dictionary.h"
#include "formats/stg_format.h"

#include <memory>

namespace kuf {

class StgEditorTab : public EditorTab {
public:
    explicit StgEditorTab(std::shared_ptr<OpenDocument> doc);

    void drawContent() override;

    void selectUnit(size_t index);
    int selectedUnit() const { return selectedUnit_; }

    const NameDictionary& nameDictionary() const { return nameDictionary_; }

private:
    enum class Section {
        Header,
        Units,
        Areas,
        Variables,
        Events
    };

    void drawSidebar();
    void drawHeaderSection();
    void drawUnitList();
    void drawUnitDetails(size_t index);
    void drawOfficerSection(const char* label, OfficerData& officer, bool active);
    void drawAreaList();
    void drawAreaDetails(size_t index);
    void drawVariableList();
    void drawVariableDetails(size_t index);
    void drawEventList();
    void drawEventDetails(size_t blockIdx, size_t eventIdx);
    void drawScriptEntry(const char* entryLabel, StgScriptEntry& entry, bool isCondition,
                         StgEvent& event, const char* dragPayloadType = nullptr,
                         int dragIndex = -1, int* dragSrc = nullptr,
                         int* dragDst = nullptr);
    void drawParamValue(const char* label, StgParamValue& param, StgEvent& event,
                        const char* paramHint = nullptr);

    Section currentSection_ = Section::Units;
    int selectedUnit_ = -1;
    int selectedArea_ = -1;
    int selectedVariable_ = -1;
    int selectedBlock_ = 0;
    int selectedEvent_ = -1;
    NameDictionary nameDictionary_;
};

} // namespace kuf
