#pragma once

#include <memory>
#include <string>
#include <vector>

namespace kuf {

class Window;
class ImGuiContext;
class HomeView;
class ValidationLogView;
class SettingsDialog;
class TabManager;
class RecentFiles;
class OpenDocument;
class EditorTab;

class Application {
public:
    Application();
    ~Application();

    void run();

private:
    void drawMenuBar();
    void drawTabBar();
    void drawDockspace();
    void openFile(const std::string& path);
    void setGameDirectory(const std::string& dir);
    void saveActiveDocument();
    void handleKeyboardShortcuts();
    void updateValidationLog();

    std::unique_ptr<Window> window_;
    std::unique_ptr<ImGuiContext> imgui_;
    std::unique_ptr<HomeView> homeView_;
    std::unique_ptr<ValidationLogView> validationLog_;
    std::unique_ptr<SettingsDialog> settingsDialog_;
    std::unique_ptr<TabManager> tabManager_;
    std::unique_ptr<RecentFiles> recentFiles_;

    std::string gameDirectory_;
    std::string pendingPopupMessage_;
    bool running_ = true;
    bool showHomeTab_ = true;
    bool showErrorPopup_ = false;
};

} // namespace kuf
