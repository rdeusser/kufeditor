#pragma once

struct GLFWwindow;
struct ImGuiIO;

namespace kuf {

class ImGuiContext {
public:
    explicit ImGuiContext(GLFWwindow* window);
    ~ImGuiContext();

    ImGuiContext(const ImGuiContext&) = delete;
    ImGuiContext& operator=(const ImGuiContext&) = delete;

    void beginFrame();
    void endFrame();

    void setFontSize(float size);

private:
    void rebuildFonts();
    void loadFont(ImGuiIO& io, float size);
    void applyDarkTheme();

    float fontSize_ = 17.0f;
    bool fontsDirty_ = false;
};

} // namespace kuf
