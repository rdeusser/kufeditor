#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"
#include "ui/views/troop_editor.h"
#include "formats/sox_binary.h"

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
}

Application::~Application() = default;

void Application::run() {
    while (running_ && !window_->shouldClose()) {
        window_->pollEvents();

        imgui_->beginFrame();

        drawDockspace();
        troopEditor_->draw();

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
    }
}

void Application::drawMenuBar() {
    if (ImGui::BeginMainMenuBar()) {
        if (ImGui::BeginMenu("File")) {
            if (ImGui::MenuItem("Open...", "Ctrl+O")) {
                // TODO: Native file dialog.
            }
            if (ImGui::MenuItem("Save", "Ctrl+S", false, currentFile_ != nullptr)) {
                if (currentFile_ && !currentPath_.empty()) {
                    auto data = currentFile_->save();
                    std::ofstream file(currentPath_, std::ios::binary);
                    file.write(reinterpret_cast<const char*>(data.data()), data.size());
                }
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Exit", "Alt+F4")) {
                running_ = false;
            }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Edit")) {
            if (ImGui::MenuItem("Undo", "Ctrl+Z", false, false)) {}
            if (ImGui::MenuItem("Redo", "Ctrl+Y", false, false)) {}
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("View")) {
            ImGui::MenuItem("Troop Editor", nullptr, &troopEditor_->isOpen());
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
    ImGui::DockSpace(dockspaceId, ImVec2(0, 0), ImGuiDockNodeFlags_PassthruCentralNode);

    // Status bar.
    ImGui::SetCursorPosY(ImGui::GetWindowHeight() - 24);
    ImGui::BeginChild("StatusBar", ImVec2(0, 24), false);
    if (currentFile_) {
        ImGui::Text("%s | %s | %zu troops",
            currentPath_.c_str(),
            currentFile_->detectedVersion() == GameVersion::Crusaders ? "Crusaders" : "Heroes",
            currentFile_->recordCount());
    } else {
        ImGui::Text("Ready");
    }
    ImGui::EndChild();

    ImGui::End();
}

} // namespace kuf
