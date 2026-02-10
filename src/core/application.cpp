#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"
#include "core/recent_files.h"
#include "core/tab_manager.h"
#include "ui/views/home_view.h"
#include "ui/views/validation_log.h"
#include "ui/views/mod_manager_view.h"
#include "ui/dialogs/file_dialog.h"
#include "ui/dialogs/settings_dialog.h"
#include "ui/tabs/editor_tab.h"
#include "ui/tabs/troop_editor_tab.h"
#include "ui/tabs/stg_editor_tab.h"
#include "formats/sox_binary.h"
#include "formats/sox_text.h"
#include "formats/stg_format.h"

#include <imgui.h>
#include <imgui_impl_opengl3.h>
#include <GLFW/glfw3.h>

#include <filesystem>
#include <fstream>

namespace kuf {

namespace {

std::string getFileName(const std::string& path) {
    auto pos = path.find_last_of("/\\");
    if (pos != std::string::npos) {
        return path.substr(pos + 1);
    }
    return path;
}

} // namespace

Application::Application() {
    window_ = std::make_unique<Window>("KUF Editor", 1280, 720);
    imgui_ = std::make_unique<ImGuiContext>(window_->handle());

    // Create views.
    homeView_ = std::make_unique<HomeView>();
    validationLog_ = std::make_unique<ValidationLogView>();
    modManagerView_ = std::make_unique<ModManagerView>();

    modManagerView_->setOnError([this](const std::string& msg) {
        pendingPopupMessage_ = msg;
        showErrorPopup_ = true;
    });

    // Create tab manager.
    tabManager_ = std::make_unique<TabManager>();

    // Create dialogs.
    settingsDialog_ = std::make_unique<SettingsDialog>();

    // Create core services.
    recentFiles_ = std::make_unique<RecentFiles>(10);

    // Load config.
    settingsDialog_->load();
    settingsDialog_->apply();
    recentFiles_->files() = settingsDialog_->config().recentFiles;
    recentFiles_->setMaxFiles(settingsDialog_->config().maxRecentFiles);

    // Set up callbacks.
    homeView_->setOnSelectGameDirectory([this](const std::string& dir) {
        setGameDirectory(dir);
    });

    tabManager_->setOnDocumentOpened([this](OpenDocument* doc) {
        if (doc && !doc->path.empty()) {
            recentFiles_->add(doc->path);
            settingsDialog_->config().recentFiles = recentFiles_->files();
            settingsDialog_->save();
        }
    });

    validationLog_->setOnNavigate([this](size_t recordIndex) {
        auto* tab = tabManager_->activeTab();
        if (auto* troopTab = dynamic_cast<TroopEditorTab*>(tab)) {
            troopTab->selectTroop(recordIndex);
        } else if (auto* stgTab = dynamic_cast<StgEditorTab*>(tab)) {
            stgTab->selectUnit(recordIndex);
        }
    });
}

Application::~Application() {
    settingsDialog_->config().recentFiles = recentFiles_->files();
    settingsDialog_->save();
}

void Application::run() {
    while (running_ && !window_->shouldClose()) {
        window_->pollEvents();

        imgui_->beginFrame();

        handleKeyboardShortcuts();
        drawDockspace();

        // Draw validation log (dockable).
        validationLog_->draw();

        // Draw dialogs.
        settingsDialog_->draw();

        // Error popup.
        if (showErrorPopup_) {
            ImGui::OpenPopup("Error");
            showErrorPopup_ = false;
        }
        if (ImGui::BeginPopupModal("Error", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
            ImGui::Text("%s", pendingPopupMessage_.c_str());
            ImGui::Separator();
            if (ImGui::Button("OK", ImVec2(120, 0))) {
                ImGui::CloseCurrentPopup();
            }
            ImGui::EndPopup();
        }

        imgui_->endFrame();

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        window_->swapBuffers();
    }
}

void Application::openFile(const std::string& path) {
    auto result = tabManager_->openFile(path);

    if (result.result == OpenResult::FileNotFound) {
        pendingPopupMessage_ = "Cannot open file: File not found";
        showErrorPopup_ = true;
    } else if (result.result == OpenResult::UnsupportedFormat) {
        pendingPopupMessage_ = "Cannot open file: Unsupported format";
        showErrorPopup_ = true;
    }

    updateValidationLog();
}

void Application::setGameDirectory(const std::string& dir) {
    gameDirectory_ = dir;
    modManagerView_->setGameDirectory(dir);
}

void Application::saveActiveDocument() {
    auto* tab = tabManager_->activeTab();
    if (tab && tab->document()) {
        tabManager_->saveDocument(tab->document().get());
    }
}

void Application::handleKeyboardShortcuts() {
    ImGuiIO& io = ImGui::GetIO();

    // Check for Ctrl/Cmd modifier.
    bool cmdOrCtrl = io.KeyCtrl;
#ifdef __APPLE__
    cmdOrCtrl = io.KeySuper;
#endif

    auto* activeTab = tabManager_->activeTab();
    auto* activeDoc = activeTab ? activeTab->document().get() : nullptr;

    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_Z)) {
        if (activeDoc && activeDoc->undoStack) {
            if (io.KeyShift) {
                activeDoc->undoStack->redo();
            } else {
                activeDoc->undoStack->undo();
            }
        }
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_Y)) {
        if (activeDoc && activeDoc->undoStack) {
            activeDoc->undoStack->redo();
        }
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_O)) {
        if (auto path = FileDialog::openFile("*.sox;*.stg", gameDirectory_.empty() ? nullptr : gameDirectory_.c_str())) {
            openFile(*path);
        }
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_S)) {
        saveActiveDocument();
    }
}

void Application::updateValidationLog() {
    auto* tab = tabManager_->activeTab();
    if (!tab || !tab->document()) {
        validationLog_->setIssues({});
        return;
    }

    auto* doc = tab->document().get();
    if (doc->binaryData) {
        validationLog_->setIssues(doc->binaryData->validate());
    } else if (doc->textData) {
        validationLog_->setIssues(doc->textData->validate());
    } else if (doc->stgData) {
        validationLog_->setIssues(doc->stgData->validate());
    } else {
        validationLog_->setIssues({});
    }
}

void Application::drawMenuBar() {
    if (ImGui::BeginMainMenuBar()) {
        if (ImGui::BeginMenu("File")) {
            if (ImGui::MenuItem("Open File...", "Ctrl+O")) {
                if (auto path = FileDialog::openFile("*.sox;*.stg", gameDirectory_.empty() ? nullptr : gameDirectory_.c_str())) {
                    openFile(*path);
                }
            }

            if (ImGui::MenuItem("Set Game Directory...")) {
                if (auto path = FileDialog::openFolder()) {
                    setGameDirectory(*path);
                }
            }

            if (ImGui::BeginMenu("Open Recent", !recentFiles_->empty())) {
                for (const auto& path : recentFiles_->files()) {
                    if (ImGui::MenuItem(getFileName(path).c_str())) {
                        openFile(path);
                    }
                    if (ImGui::IsItemHovered()) {
                        ImGui::SetTooltip("%s", path.c_str());
                    }
                }
                ImGui::Separator();
                if (ImGui::MenuItem("Clear Recent Files")) {
                    recentFiles_->clear();
                }
                ImGui::EndMenu();
            }

            auto* activeTab = tabManager_->activeTab();
            bool hasFile = activeTab && activeTab->document() && activeTab->document()->hasData();
            if (ImGui::MenuItem("Save", "Ctrl+S", false, hasFile)) {
                saveActiveDocument();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Exit", "Alt+F4")) {
                running_ = false;
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("Edit")) {
            auto* activeTab = tabManager_->activeTab();
            auto* activeDoc = activeTab ? activeTab->document().get() : nullptr;
            auto* undoStack = activeDoc ? activeDoc->undoStack.get() : nullptr;

            std::string undoLabel = "Undo";
            std::string redoLabel = "Redo";
            bool canUndo = undoStack && undoStack->canUndo();
            bool canRedo = undoStack && undoStack->canRedo();

            if (canUndo) {
                undoLabel += " " + undoStack->undoDescription();
            }
            if (canRedo) {
                redoLabel += " " + undoStack->redoDescription();
            }

            if (ImGui::MenuItem(undoLabel.c_str(), "Ctrl+Z", false, canUndo)) {
                undoStack->undo();
            }
            if (ImGui::MenuItem(redoLabel.c_str(), "Ctrl+Y", false, canRedo)) {
                undoStack->redo();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Restore from Backup...", nullptr, false, !gameDirectory_.empty())) {
                modManagerView_->restoreLatestBackup();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Settings...")) {
                settingsDialog_->open();
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("View")) {
            ImGui::MenuItem("Home", nullptr, &showHomeTab_);
            ImGui::MenuItem("Mod Manager", nullptr, &showModManager_);
            ImGui::MenuItem("Validation Log", nullptr, &validationLog_->isOpen());
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("Help")) {
            if (ImGui::MenuItem("About")) {}
            ImGui::EndMenu();
        }

        ImGui::EndMainMenuBar();
    }
}

void Application::drawTabBar() {
    ImGuiTabBarFlags tabBarFlags = ImGuiTabBarFlags_Reorderable |
                                   ImGuiTabBarFlags_AutoSelectNewTabs |
                                   ImGuiTabBarFlags_FittingPolicyScroll;

    enum class ActiveContent { None, Home, ModManager, Editor };
    ActiveContent activeContent = ActiveContent::None;
    EditorTab* activeEditorTab = nullptr;
    EditorTab* tabToClose = nullptr;

    if (ImGui::BeginTabBar("MainTabBar", tabBarFlags)) {
        // Home tab header.
        if (showHomeTab_) {
            bool homeOpen = true;
            if (ImGui::BeginTabItem("Home", &homeOpen)) {
                activeContent = ActiveContent::Home;
                ImGui::EndTabItem();
            }
            if (!homeOpen) {
                showHomeTab_ = false;
            }
        }

        // Mod Manager tab header.
        if (showModManager_) {
            bool modOpen = true;
            if (ImGui::BeginTabItem("Mod Manager", &modOpen)) {
                activeContent = ActiveContent::ModManager;
                ImGui::EndTabItem();
            }
            if (!modOpen) {
                showModManager_ = false;
            }
        }

        // Editor tab headers.
        for (const auto& tab : tabManager_->tabs()) {
            ImGuiTabItemFlags flags = ImGuiTabItemFlags_None;
            bool open = tab->isOpen();

            if (tab->document() && tab->document()->dirty) {
                flags |= ImGuiTabItemFlags_UnsavedDocument;
            }

            ImGui::PushID(tab->tabId());
            if (ImGui::BeginTabItem(tab->tabTitle().c_str(), &open, flags)) {
                if (tabManager_->activeTab() != tab.get()) {
                    tabManager_->setActiveTab(tab.get());
                    updateValidationLog();
                }
                activeContent = ActiveContent::Editor;
                activeEditorTab = tab.get();
                ImGui::EndTabItem();
            }
            ImGui::PopID();

            if (!open) {
                tabToClose = tab.get();
            }
        }

        ImGui::EndTabBar();
    }

    // Draw active tab content in a child window that reserves 24px for the status bar.
    ImGui::BeginChild("TabContent", ImVec2(0, -24.0f));
    switch (activeContent) {
    case ActiveContent::Home:
        homeView_->drawContent();
        break;
    case ActiveContent::ModManager:
        modManagerView_->drawContent();
        break;
    case ActiveContent::Editor:
        if (activeEditorTab) {
            activeEditorTab->drawContent();
        }
        break;
    case ActiveContent::None:
        break;
    }
    ImGui::EndChild();

    if (tabToClose) {
        tabManager_->closeTab(tabToClose);
        updateValidationLog();
    }
}

void Application::drawDockspace() {
    ImGuiViewport* viewport = ImGui::GetMainViewport();
    ImGui::SetNextWindowPos(viewport->WorkPos);
    ImGui::SetNextWindowSize(viewport->WorkSize);
    ImGui::SetNextWindowViewport(viewport->ID);

    ImGuiWindowFlags flags =
        ImGuiWindowFlags_NoDocking |
        ImGuiWindowFlags_NoTitleBar |
        ImGuiWindowFlags_NoCollapse |
        ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove |
        ImGuiWindowFlags_NoBringToFrontOnFocus |
        ImGuiWindowFlags_NoNavFocus |
        ImGuiWindowFlags_NoBackground |
        ImGuiWindowFlags_MenuBar |
        ImGuiWindowFlags_NoScrollbar |
        ImGuiWindowFlags_NoScrollWithMouse;

    ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));

    ImGui::Begin("MainDockspaceWindow", nullptr, flags);
    ImGui::PopStyleVar(3);

    drawMenuBar();

    // Tab bar area.
    ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(8, 6));
    drawTabBar();
    ImGui::PopStyleVar();

    // Status bar.
    ImGui::BeginChild("StatusBar", ImVec2(0, 24.0f), false);
    ImGui::SetCursorPosX(8.0f);

    auto* activeTab = tabManager_->activeTab();
    if (activeTab && activeTab->document()) {
        auto* doc = activeTab->document().get();
        if (doc->binaryData) {
            ImGui::Text("%s%s | %s | %zu troops",
                doc->path.c_str(),
                doc->dirty ? "*" : "",
                doc->binaryData->detectedVersion() == GameVersion::Crusaders ? "Crusaders" : "Heroes",
                doc->binaryData->recordCount());
        } else if (doc->textData) {
            ImGui::Text("%s%s | Text SOX | %zu entries",
                doc->path.c_str(),
                doc->dirty ? "*" : "",
                doc->textData->entryCount());
        } else if (doc->stgData) {
            ImGui::Text("%s%s | STG Mission | %zu units",
                doc->path.c_str(),
                doc->dirty ? "*" : "",
                doc->stgData->unitCount());
        } else {
            ImGui::Text("%s | Unknown format | %zu bytes",
                doc->path.c_str(),
                doc->rawData.size());
        }
    } else if (showHomeTab_) {
        ImGui::Text("Ready - Select a game or open a file");
    } else {
        ImGui::Text("Ready");
    }
    ImGui::EndChild();

    ImGui::End();
}

} // namespace kuf
