#pragma once

#include "ui/tabs/editor_tab.h"
#include "formats/sox_text.h"

#include <memory>

namespace kuf {

class TextEditorTab : public EditorTab {
public:
    explicit TextEditorTab(std::shared_ptr<OpenDocument> doc);

    void drawContent() override;

private:
    int selectedEntry_ = -1;
    char editBuffer_[256] = {};
};

} // namespace kuf
