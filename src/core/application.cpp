#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"
#include "core/recent_files.h"
#include "ui/views/troop_editor.h"
#include "ui/views/text_editor.h"
#include "ui/views/validation_log.h"
#include "ui/dialogs/file_dialog.h"
#include "ui/dialogs/settings_dialog.h"
#include "formats/sox_binary.h"
#include "formats/sox_text.h"
#include "formats/sox_encoding.h"
#include "undo/undo_stack.h"

#include <imgui.h>
#include <imgui_impl_opengl3.h>
#include <GLFW/glfw3.h>

#include <fstream>
#include <filesystem>

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

    // Create all views.
    troopEditor_ = std::make_unique<TroopEditorView>();
    textEditor_ = std::make_unique<TextEditorView>();
    validationLog_ = std::make_unique<ValidationLogView>();

    // Create dialogs.
    settingsDialog_ = std::make_unique<SettingsDialog>();

    // Create core services.
    undoStack_ = std::make_unique<UndoStack>();
    recentFiles_ = std::make_unique<RecentFiles>(10);

    // Load config.
    settingsDialog_->load();
    settingsDialog_->apply();
    recentFiles_->files() = settingsDialog_->config().recentFiles;
    recentFiles_->setMaxFiles(settingsDialog_->config().maxRecentFiles);

    // Set up callbacks.
    undoStack_->setOnChange([this]() {
        dirty_ = true;
    });

    validationLog_->setOnNavigate([this](size_t recordIndex) {
        troopEditor_->selectTroop(recordIndex);
        troopEditor_->isOpen() = true;
    });
}

Application::~Application() {
    // Save config.
    settingsDialog_->config().recentFiles = recentFiles_->files();
    settingsDialog_->save();
}

void Application::run() {
    while (running_ && !window_->shouldClose()) {
        window_->pollEvents();

        imgui_->beginFrame();

        handleKeyboardShortcuts();
        drawDockspace();

        // Draw all views.
        troopEditor_->draw();
        textEditor_->draw();
        validationLog_->draw();

        // Draw dialogs.
        settingsDialog_->draw();

        imgui_->endFrame();

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        window_->swapBuffers();
    }
}

void Application::openFile(const std::string& path) {
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file) return;

    auto size = file.tellg();
    file.seekg(0);

    rawFileData_.resize(size);
    file.read(reinterpret_cast<char*>(rawFileData_.data()), size);

    std::string filename = getFileName(path);

    // SOX files use ASCII hex encoding. Decode if detected.
    std::span<const std::byte> parseData = rawFileData_;
    std::vector<std::byte> decodedData;
    isSoxEncoded_ = isSoxEncoded(rawFileData_);

    if (isSoxEncoded_) {
        auto decoded = soxDecode(rawFileData_);
        if (decoded) {
            decodedData = std::move(*decoded);
            parseData = decodedData;
        }
    }

    // Try binary SOX first (has version header = 100).
    auto binary = std::make_shared<SoxBinary>();
    if (binary->load(parseData)) {
        currentBinaryFile_ = binary;
        currentTextFile_ = nullptr;
        currentPath_ = path;
        troopEditor_->setData(binary);
        textEditor_->setData(nullptr);
        validationLog_->setIssues(binary->validate());
        undoStack_->clear();
        dirty_ = false;
        recentFiles_->add(path);
        return;
    }

    // Try text SOX.
    auto text = std::make_shared<SoxText>();
    if (text->load(parseData)) {
        currentTextFile_ = text;
        currentBinaryFile_ = nullptr;
        currentPath_ = path;
        textEditor_->setData(text);
        troopEditor_->setData(nullptr);
        validationLog_->setIssues(text->validate());
        undoStack_->clear();
        dirty_ = false;
        recentFiles_->add(path);
        return;
    }

    // Unknown format.
    currentBinaryFile_ = nullptr;
    currentTextFile_ = nullptr;
    currentPath_ = path;
    troopEditor_->setData(nullptr);
    textEditor_->setData(nullptr);
    validationLog_->setIssues({});
    undoStack_->clear();
    dirty_ = false;
    recentFiles_->add(path);
}

void Application::saveFile() {
    if (currentPath_.empty()) return;

    std::vector<std::byte> data;
    if (currentBinaryFile_) {
        data = currentBinaryFile_->save();
    } else if (currentTextFile_) {
        data = currentTextFile_->save();
    }

    if (!data.empty()) {
        // Re-encode to ASCII hex if the original file was hex-encoded.
        if (isSoxEncoded_) {
            data = soxEncode(data);
        }

        std::ofstream file(currentPath_, std::ios::binary);
        file.write(reinterpret_cast<const char*>(data.data()), data.size());
        dirty_ = false;
    }
}

void Application::handleKeyboardShortcuts() {
    ImGuiIO& io = ImGui::GetIO();

    // Check for Ctrl/Cmd modifier.
    bool cmdOrCtrl = io.KeyCtrl;
#ifdef __APPLE__
    cmdOrCtrl = io.KeySuper;
#endif

    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_Z)) {
        if (io.KeyShift) {
            undoStack_->redo();
        } else {
            undoStack_->undo();
        }
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_Y)) {
        undoStack_->redo();
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_O)) {
        if (auto path = FileDialog::openFile("*.sox")) {
            openFile(*path);
        }
    }
    if (cmdOrCtrl && ImGui::IsKeyPressed(ImGuiKey_S)) {
        saveFile();
    }
}

void Application::drawMenuBar() {
    if (ImGui::BeginMainMenuBar()) {
        if (ImGui::BeginMenu("File")) {
            if (ImGui::MenuItem("Open...", "Ctrl+O")) {
                if (auto path = FileDialog::openFile("*.sox")) {
                    openFile(*path);
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

            bool hasFile = currentBinaryFile_ || currentTextFile_;
            if (ImGui::MenuItem("Save", "Ctrl+S", false, hasFile)) {
                saveFile();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Exit", "Alt+F4")) {
                running_ = false;
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("Edit")) {
            std::string undoLabel = "Undo";
            std::string redoLabel = "Redo";
            if (undoStack_->canUndo()) {
                undoLabel += " " + undoStack_->undoDescription();
            }
            if (undoStack_->canRedo()) {
                redoLabel += " " + undoStack_->redoDescription();
            }

            if (ImGui::MenuItem(undoLabel.c_str(), "Ctrl+Z", false, undoStack_->canUndo())) {
                undoStack_->undo();
            }
            if (ImGui::MenuItem(redoLabel.c_str(), "Ctrl+Y", false, undoStack_->canRedo())) {
                undoStack_->redo();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Settings...")) {
                settingsDialog_->open();
            }
            ImGui::EndMenu();
        }

        if (ImGui::BeginMenu("View")) {
            ImGui::MenuItem("Troop Editor", nullptr, &troopEditor_->isOpen());
            ImGui::MenuItem("Text Editor", nullptr, &textEditor_->isOpen());
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
        ImGuiWindowFlags_MenuBar;

    ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));

    ImGui::Begin("MainDockspaceWindow", nullptr, flags);
    ImGui::PopStyleVar(3);

    drawMenuBar();

    // Reserve space for status bar at bottom.
    float statusBarHeight = 24.0f;
    float dockspaceHeight = ImGui::GetContentRegionAvail().y - statusBarHeight;

    ImGuiID dockspaceId = ImGui::GetID("MainDockspace");
    ImGui::DockSpace(dockspaceId, ImVec2(0, dockspaceHeight), ImGuiDockNodeFlags_None);

    // Status bar.
    ImGui::BeginChild("StatusBar", ImVec2(0, statusBarHeight), false);
    ImGui::SetCursorPosX(8.0f);
    if (currentBinaryFile_) {
        ImGui::Text("%s%s | %s | %zu troops",
            currentPath_.c_str(),
            dirty_ ? "*" : "",
            currentBinaryFile_->detectedVersion() == GameVersion::Crusaders ? "Crusaders" : "Heroes",
            currentBinaryFile_->recordCount());
    } else if (currentTextFile_) {
        ImGui::Text("%s%s | Text SOX | %zu entries",
            currentPath_.c_str(),
            dirty_ ? "*" : "",
            currentTextFile_->entryCount());
    } else if (!currentPath_.empty()) {
        ImGui::Text("%s | Unknown format | %zu bytes",
            currentPath_.c_str(),
            rawFileData_.size());
    } else {
        ImGui::Text("Ready");
    }
    ImGui::EndChild();

    ImGui::End();
}

} // namespace kuf
