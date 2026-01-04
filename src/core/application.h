#pragma once

#include <memory>
#include <string>

namespace kuf {

class Window;
class ImGuiContext;

class Application {
public:
    Application();
    ~Application();

    void run();

private:
    void drawMenuBar();
    void drawDockspace();

    std::unique_ptr<Window> window_;
    std::unique_ptr<ImGuiContext> imgui_;
    bool running_ = true;
};

} // namespace kuf
