#include "core/window.h"
#include "core/imgui_context.h"

#include <GLFW/glfw3.h>
#include <imgui.h>
#include <imgui_impl_opengl3.h>

int main() {
    kuf::Window window("KUF Editor", 1280, 720);
    kuf::ImGuiContext imgui(window.handle());

    while (!window.shouldClose()) {
        window.pollEvents();

        imgui.beginFrame();

        // Show demo window for testing.
        ImGui::ShowDemoWindow();

        imgui.endFrame();

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        window.swapBuffers();
    }

    return 0;
}
