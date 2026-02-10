#pragma once

#include <imgui.h>
#include <imgui_internal.h>
#include <algorithm>
#include <cstring>

namespace kuf {

inline bool InputTextCentered(const char* label, char* buf, size_t bufSize,
                              ImGuiInputTextFlags flags = 0,
                              ImGuiInputTextCallback callback = nullptr,
                              void* userData = nullptr) {
    const ImGuiContext& g = *ImGui::GetCurrentContext();
    const ImGuiStyle& style = ImGui::GetStyle();
    ImGuiWindow* window = ImGui::GetCurrentWindow();
    ImGuiID id = window->GetID(label);
    bool active = g.ActiveId == id;

    // When not active, suppress InputText's own text rendering.
    if (!active) {
        ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(0, 0, 0, 0));
    }

    bool modified = ImGui::InputText(label, buf, bufSize, flags, callback, userData);

    if (!active) {
        ImGui::PopStyleColor();

        ImVec2 frameMin = ImGui::GetItemRectMin();
        float frameHeight = ImGui::GetItemRectMax().y - frameMin.y;
        float frameWidth = ImGui::CalcItemWidth();

        ImVec2 textSize = ImGui::CalcTextSize(buf);
        float centeredX = frameMin.x + (frameWidth - textSize.x) * 0.5f;
        float minX = frameMin.x + style.FramePadding.x;
        ImVec2 textPos(std::max(centeredX, minX), frameMin.y + style.FramePadding.y);

        ImDrawList* drawList = ImGui::GetWindowDrawList();
        ImVec4 clipRect(frameMin.x, frameMin.y,
                        frameMin.x + frameWidth, frameMin.y + frameHeight);
        ImU32 textCol = ImColor(style.Colors[ImGuiCol_Text]);
        drawList->AddText(nullptr, 0.0f, textPos, textCol,
                          buf, nullptr, 0.0f, &clipRect);

        // Re-render the label (also made invisible by PushStyleColor).
        const char* labelEnd = std::strstr(label, "##");
        ImVec2 labelSize = ImGui::CalcTextSize(label, labelEnd, true);
        if (labelSize.x > 0.0f) {
            float labelX = frameMin.x + frameWidth + style.ItemInnerSpacing.x;
            ImVec2 labelPos(labelX, frameMin.y + style.FramePadding.y);
            drawList->AddText(labelPos, textCol, label, labelEnd);
        }
    }

    return modified;
}

inline bool BeginComboCentered(const char* label, const char* previewValue,
                               ImGuiComboFlags flags = 0) {
    const ImGuiStyle& style = ImGui::GetStyle();

    ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(0, 0, 0, 0));
    bool open = ImGui::BeginCombo(label, previewValue, flags);
    ImGui::PopStyleColor();

    ImVec2 frameMin = ImGui::GetItemRectMin();
    float frameHeight = ImGui::GetItemRectMax().y - frameMin.y;
    float frameWidth = ImGui::CalcItemWidth();
    float arrowWidth = ImGui::GetFrameHeight();
    float availWidth = frameWidth - arrowWidth;

    ImVec2 textSize = ImGui::CalcTextSize(previewValue);
    float centeredX = frameMin.x + (frameWidth - textSize.x) * 0.5f;
    float minX = frameMin.x + style.FramePadding.x;
    ImVec2 textPos(std::max(centeredX, minX), frameMin.y + style.FramePadding.y);

    ImDrawList* drawList = ImGui::GetWindowDrawList();
    ImVec4 clipRect(frameMin.x, frameMin.y,
                    frameMin.x + availWidth, frameMin.y + frameHeight);
    ImU32 textCol = ImColor(style.Colors[ImGuiCol_Text]);
    drawList->AddText(nullptr, 0.0f, textPos, textCol,
                      previewValue, nullptr, 0.0f, &clipRect);

    const char* labelEnd = std::strstr(label, "##");
    ImVec2 labelSize = ImGui::CalcTextSize(label, labelEnd, true);
    if (labelSize.x > 0.0f) {
        float labelX = frameMin.x + frameWidth + style.ItemInnerSpacing.x;
        ImVec2 labelPos(labelX, frameMin.y + style.FramePadding.y);
        drawList->AddText(labelPos, textCol, label, labelEnd);
    }

    return open;
}

inline bool ComboCentered(const char* label, int* currentItem,
                           const char* const items[], int itemsCount,
                           ImGuiComboFlags flags = 0) {
    const char* previewValue = (*currentItem >= 0 && *currentItem < itemsCount)
        ? items[*currentItem] : "";

    bool changed = false;
    if (BeginComboCentered(label, previewValue, flags)) {
        for (int i = 0; i < itemsCount; ++i) {
            bool selected = (i == *currentItem);
            if (ImGui::Selectable(items[i], selected)) {
                *currentItem = i;
                changed = true;
            }
            if (selected) ImGui::SetItemDefaultFocus();
        }
        ImGui::EndCombo();
    }
    return changed;
}

} // namespace kuf
