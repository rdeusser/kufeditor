#pragma once

struct GLFWwindow;

namespace kuf {

class ImGuiContext {
public:
    explicit ImGuiContext(GLFWwindow* window);
    ~ImGuiContext();

    ImGuiContext(const ImGuiContext&) = delete;
    ImGuiContext& operator=(const ImGuiContext&) = delete;

    void beginFrame();
    void endFrame();

private:
    void applyDarkTheme();
};

} // namespace kuf
