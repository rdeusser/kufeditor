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

} // namespace kuf
