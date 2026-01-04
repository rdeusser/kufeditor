#pragma once

#include <memory>
#include <string>

namespace kuf {

class Window;
class ImGuiContext;
class TroopEditorView;
class SoxBinary;

class Application {
public:
    Application();
    ~Application();

    void run();

private:
    void drawMenuBar();
    void drawDockspace();
    void openFile(const std::string& path);

    std::unique_ptr<Window> window_;
    std::unique_ptr<ImGuiContext> imgui_;
    std::unique_ptr<TroopEditorView> troopEditor_;
    std::shared_ptr<SoxBinary> currentFile_;
    std::string currentPath_;
    bool running_ = true;
};

} // namespace kuf
