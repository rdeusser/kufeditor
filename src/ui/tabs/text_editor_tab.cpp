#include "ui/tabs/text_editor_tab.h"

#include <imgui.h>
#include <algorithm>
#include <cstring>

namespace kuf {

TextEditorTab::TextEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {
    editBuffer_[0] = '\0';
}

void TextEditorTab::drawContent() {
    if (!document_ || !document_->textData) {
        ImGui::TextDisabled("No text file loaded");
        return;
    }

    auto& textData = document_->textData;
    ImGui::Text("%zu text entries", textData->entryCount());
    ImGui::Separator();

    ImGui::BeginChild("TextList", ImVec2(0, 0), true);

    if (ImGui::BeginTable("TextTable", 3,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg | ImGuiTableFlags_ScrollY)) {

        ImGui::TableSetupColumn("#", ImGuiTableColumnFlags_WidthFixed, 50.0f);
        ImGui::TableSetupColumn("Max", ImGuiTableColumnFlags_WidthFixed, 40.0f);
        ImGui::TableSetupColumn("Text", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < textData->entries().size(); ++i) {
            auto& entry = textData->entries()[i];
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
                    document_->dirty = true;
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
