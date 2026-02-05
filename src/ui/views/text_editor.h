#pragma once

#include "ui/views/view.h"
#include "formats/sox_text.h"

#include <memory>

namespace kuf {

class TextEditorView : public View {
public:
    TextEditorView();

    void drawContent() override;

    void setData(std::shared_ptr<SoxText> data);
    bool hasData() const { return data_ != nullptr; }

private:
    std::shared_ptr<SoxText> data_;
    int selectedEntry_ = -1;
    char editBuffer_[256] = {};
};

} // namespace kuf
