#pragma once

#include <memory>
#include <string>
#include <vector>

namespace kuf {

class Window;
class ImGuiContext;
class TroopEditorView;
class TextEditorView;
class ValidationLogView;
class SettingsDialog;
class SoxBinary;
class SoxText;
class UndoStack;
class RecentFiles;

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
    void saveFile();
    void handleKeyboardShortcuts();
    void updateRecentFilesMenu();

    std::unique_ptr<Window> window_;
    std::unique_ptr<ImGuiContext> imgui_;
    std::unique_ptr<TroopEditorView> troopEditor_;
    std::unique_ptr<TextEditorView> textEditor_;
    std::unique_ptr<ValidationLogView> validationLog_;
    std::unique_ptr<SettingsDialog> settingsDialog_;
    std::unique_ptr<UndoStack> undoStack_;
    std::unique_ptr<RecentFiles> recentFiles_;

    std::shared_ptr<SoxBinary> currentBinaryFile_;
    std::shared_ptr<SoxText> currentTextFile_;
    std::vector<std::byte> rawFileData_;
    std::string currentPath_;
    bool running_ = true;
    bool dirty_ = false;
    bool isSoxEncoded_ = false;
};

} // namespace kuf
