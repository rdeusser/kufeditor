#pragma once

#include <memory>
#include <string>

namespace kuf {

class Window;
class ImGuiContext;
class TroopEditorView;
class ValidationLogView;
class SoxBinary;
class UndoStack;

class Application {
public:
    Application();
    ~Application();

    void run();

    UndoStack& undoStack() { return *undoStack_; }

private:
    void drawMenuBar();
    void drawDockspace();
    void openFile(const std::string& path);
    void handleKeyboardShortcuts();

    std::unique_ptr<Window> window_;
    std::unique_ptr<ImGuiContext> imgui_;
    std::unique_ptr<TroopEditorView> troopEditor_;
    std::unique_ptr<ValidationLogView> validationLog_;
    std::unique_ptr<UndoStack> undoStack_;
    std::shared_ptr<SoxBinary> currentFile_;
    std::string currentPath_;
    bool running_ = true;
    bool dirty_ = false;
};

} // namespace kuf
