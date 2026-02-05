#include "ui/views/text_editor.h"

#include <imgui.h>
#include <algorithm>
#include <cstring>

namespace kuf {

TextEditorView::TextEditorView() : View("Text Editor") {}

void TextEditorView::setData(std::shared_ptr<SoxText> data) {
    data_ = std::move(data);
    selectedEntry_ = -1;
    editBuffer_[0] = '\0';
}

void TextEditorView::drawContent() {
    if (!data_) {
        ImGui::TextDisabled("No text file loaded");
        return;
    }

    ImGui::Text("%zu text entries", data_->entryCount());
    ImGui::Separator();

    ImGui::BeginChild("TextList", ImVec2(0, 0), true);

    if (ImGui::BeginTable("TextTable", 3,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg | ImGuiTableFlags_ScrollY)) {

        ImGui::TableSetupColumn("#", ImGuiTableColumnFlags_WidthFixed, 50.0f);
        ImGui::TableSetupColumn("Max", ImGuiTableColumnFlags_WidthFixed, 40.0f);
        ImGui::TableSetupColumn("Text", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < data_->entries().size(); ++i) {
            auto& entry = data_->entries()[i];
            ImGui::TableNextRow();

            // Index.
            ImGui::TableNextColumn();
            char label[32];
            snprintf(label, sizeof(label), "%zu", i);
            bool selected = (selectedEntry_ == static_cast<int>(i));
            if (ImGui::Selectable(label, selected, ImGuiSelectableFlags_SpanAllColumns)) {
                selectedEntry_ = static_cast<int>(i);
                strncpy(editBuffer_, entry.text.c_str(), sizeof(editBuffer_) - 1);
                editBuffer_[sizeof(editBuffer_) - 1] = '\0';
            }

            // Max length.
            ImGui::TableNextColumn();
            ImGui::Text("%d", entry.maxLength);

            // Text content.
            ImGui::TableNextColumn();
            if (selected) {
                ImGui::SetNextItemWidth(-1);
                if (ImGui::InputText("##edit", editBuffer_, entry.maxLength + 1,
                        ImGuiInputTextFlags_EnterReturnsTrue)) {
                    entry.text = editBuffer_;
                }
            } else {
                ImGui::Text("%s", entry.text.c_str());
            }
        }

        ImGui::EndTable();
    }

    ImGui::EndChild();
}

} // namespace kuf
