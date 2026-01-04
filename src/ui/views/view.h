#pragma once

#include <imgui.h>

#include <string>

namespace kuf {

class View {
public:
    explicit View(std::string name) : name_(std::move(name)) {}
    virtual ~View() = default;

    virtual void drawContent() = 0;

    void draw() {
        if (!open_) return;

        ImGui::SetNextWindowSizeConstraints(minSize(), maxSize());

        if (ImGui::Begin(name_.c_str(), &open_, flags())) {
            drawContent();
        }
        ImGui::End();
    }

    bool& isOpen() { return open_; }
    const std::string& name() const { return name_; }

protected:
    virtual ImGuiWindowFlags flags() const { return ImGuiWindowFlags_None; }
    virtual ImVec2 minSize() const { return {200, 100}; }
    virtual ImVec2 maxSize() const { return {FLT_MAX, FLT_MAX}; }

    std::string name_;
    bool open_ = true;
};

} // namespace kuf
