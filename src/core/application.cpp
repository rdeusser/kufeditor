#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"
#include "ui/views/troop_editor.h"
#include "ui/views/validation_log.h"
#include "ui/dialogs/file_dialog.h"
#include "formats/sox_binary.h"
#include "undo/undo_stack.h"

#include <imgui.h>
#include <imgui_impl_opengl3.h>
#include <GLFW/glfw3.h>

#include <fstream>
#include <vector>

namespace kuf {

Application::Application() {
    window_ = std::make_unique<Window>("KUF Editor", 1280, 720);
    imgui_ = std::make_unique<ImGuiContext>(window_->handle());
    troopEditor_ = std::make_unique<TroopEditorView>();
    validationLog_ = std::make_unique<ValidationLogView>();
    undoStack_ = std::make_unique<UndoStack>();

    undoStack_->setOnChange([this]() {
        dirty_ = true;
    });

    validationLog_->setOnNavigate([this](size_t recordIndex) {
        troopEditor_->selectTroop(recordIndex);
        troopEditor_->isOpen() = true;
    });
}

Application::~Application() = default;

void Application::run() {
    while (running_ && !window_->shouldClose()) {
        window_->pollEvents();

        imgui_->beginFrame();

        handleKeyboardShortcuts();
        drawDockspace();
        troopEditor_->draw();
        validationLog_->draw();

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

    std::vector<std::byte> data(size);
    file.read(reinterpret_cast<char*>(data.data()), size);

    auto sox = std::make_shared<SoxBinary>();
    if (sox->load(data)) {
        currentFile_ = sox;
        currentPath_ = path;
        troopEditor_->setData(sox);
        validationLog_->setIssues(sox->validate());
        undoStack_->clear();
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
        if (currentFile_ && !currentPath_.empty()) {
            auto data = currentFile_->save();
            std::ofstream file(currentPath_, std::ios::binary);
            file.write(reinterpret_cast<const char*>(data.data()), data.size());
            dirty_ = false;
        }
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
            if (ImGui::MenuItem("Save", "Ctrl+S", false, currentFile_ != nullptr)) {
                if (currentFile_ && !currentPath_.empty()) {
                    auto data = currentFile_->save();
                    std::ofstream file(currentPath_, std::ios::binary);
                    file.write(reinterpret_cast<const char*>(data.data()), data.size());
                    dirty_ = false;
                }
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
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("View")) {
            ImGui::MenuItem("Troop Editor", nullptr, &troopEditor_->isOpen());
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

    ImGuiID dockspaceId = ImGui::GetID("MainDockspace");
    ImGui::DockSpace(dockspaceId, ImVec2(0, 0), ImGuiDockNodeFlags_None);

    // Status bar.
    ImGui::SetCursorPosY(ImGui::GetWindowHeight() - 24);
    ImGui::BeginChild("StatusBar", ImVec2(0, 24), false);
    ImGui::SetCursorPosX(8.0f);  // left padding
    if (currentFile_) {
        ImGui::Text("%s%s | %s | %zu troops",
            currentPath_.c_str(),
            dirty_ ? "*" : "",
            currentFile_->detectedVersion() == GameVersion::Crusaders ? "Crusaders" : "Heroes",
            currentFile_->recordCount());
    } else {
        ImGui::Text("Ready");
    }
    ImGui::EndChild();

    ImGui::End();
}

} // namespace kuf
