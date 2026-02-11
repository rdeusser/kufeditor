#include "ui/views/validation_log.h"

#include <imgui.h>

namespace kuf {

ValidationLogView::ValidationLogView() : View("Validation Log") {
    open_ = false;
}

void ValidationLogView::setIssues(std::vector<ValidationIssue> issues) {
    issues_ = std::move(issues);
}

void ValidationLogView::clear() {
    issues_.clear();
}

void ValidationLogView::drawContent() {
    if (issues_.empty()) {
        ImGui::TextDisabled("No validation issues");
        return;
    }

    ImGui::Text("%zu issue(s) found", issues_.size());
    ImGui::Separator();

    if (ImGui::BeginTable("ValidationTable", 4,
            ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg | ImGuiTableFlags_ScrollY)) {

        ImGui::TableSetupColumn("", ImGuiTableColumnFlags_WidthFixed, 24.0f);
        ImGui::TableSetupColumn("Record", ImGuiTableColumnFlags_WidthFixed, 60.0f);
        ImGui::TableSetupColumn("Field", ImGuiTableColumnFlags_WidthFixed, 120.0f);
        ImGui::TableSetupColumn("Message", ImGuiTableColumnFlags_WidthStretch);
        ImGui::TableHeadersRow();

        for (size_t i = 0; i < issues_.size(); ++i) {
            const auto& issue = issues_[i];
            ImGui::TableNextRow();
            ImGui::PushID(static_cast<int>(i));

            // Severity icon.
            ImGui::TableNextColumn();
            ImGui::TextColored(severityColor(issue.severity), "%s", severityIcon(issue.severity));

            // Record index.
            ImGui::TableNextColumn();
            char label[32];
            snprintf(label, sizeof(label), "#%zu", issue.recordIndex);
            if (ImGui::Selectable(label, false, ImGuiSelectableFlags_SpanAllColumns)) {
                if (onNavigate_) {
                    onNavigate_(issue.recordIndex);
                }
            }
            ImGui::PopID();

            // Field name.
            ImGui::TableNextColumn();
            ImGui::Text("%s", issue.field.c_str());

            // Message.
            ImGui::TableNextColumn();
            ImGui::TextWrapped("%s", issue.message.c_str());
        }

        ImGui::EndTable();
    }
}

const char* ValidationLogView::severityIcon(Severity severity) const {
    switch (severity) {
        case Severity::Info:    return "[i]";
        case Severity::Warning: return "[!]";
        case Severity::Error:   return "[X]";
    }
    return "[ ]";
}

ImVec4 ValidationLogView::severityColor(Severity severity) const {
    switch (severity) {
        case Severity::Info:    return ImVec4(0.4f, 0.7f, 1.0f, 1.0f);  // blue
        case Severity::Warning: return ImVec4(1.0f, 0.8f, 0.2f, 1.0f);  // yellow
        case Severity::Error:   return ImVec4(1.0f, 0.3f, 0.3f, 1.0f);  // red
    }
    return ImVec4(1.0f, 1.0f, 1.0f, 1.0f);
}

} // namespace kuf
