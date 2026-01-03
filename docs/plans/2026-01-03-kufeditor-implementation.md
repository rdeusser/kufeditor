# Implementation Plan: KUF Editor v1.0

## Overview

Build a desktop editor for Kingdom Under Fire game files using Dear ImGui, GLFW, and OpenGL. This plan covers the complete v1.0 implementation.

## Prerequisites

- [ ] Design document approved (`docs/plans/2026-01-03-kufeditor-design.md`)
- [ ] Visual Studio 2022 installed with C++ workload
- [ ] CMake 3.20+ installed
- [ ] Git configured

---

## Phase 1: Project Setup

### Task 1.1: Create CMake Build System

**Files:**
- Create: `CMakeLists.txt`
- Create: `src/main.cpp`

**Steps:**
1. Create root CMakeLists.txt with C++20, FetchContent for dependencies
2. Create minimal main.cpp that returns 0
3. Configure and build to verify setup works
4. Commit: "Initialize CMake build system"

**Code:**

`CMakeLists.txt`:
```cmake
cmake_minimum_required(VERSION 3.20)
project(kufeditor VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Dependencies via FetchContent
include(FetchContent)

# GLFW
FetchContent_Declare(
    glfw
    GIT_REPOSITORY https://github.com/glfw/glfw.git
    GIT_TAG 3.4
)
set(GLFW_BUILD_DOCS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(glfw)

# Dear ImGui
FetchContent_Declare(
    imgui
    GIT_REPOSITORY https://github.com/ocornut/imgui.git
    GIT_TAG docking
)
FetchContent_MakeAvailable(imgui)

# nlohmann/json
FetchContent_Declare(
    json
    GIT_REPOSITORY https://github.com/nlohmann/json.git
    GIT_TAG v3.11.3
)
FetchContent_MakeAvailable(json)

# Catch2 for testing
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.5.2
)
FetchContent_MakeAvailable(Catch2)

# ImGui library (manual setup required for imgui)
add_library(imgui STATIC
    ${imgui_SOURCE_DIR}/imgui.cpp
    ${imgui_SOURCE_DIR}/imgui_demo.cpp
    ${imgui_SOURCE_DIR}/imgui_draw.cpp
    ${imgui_SOURCE_DIR}/imgui_tables.cpp
    ${imgui_SOURCE_DIR}/imgui_widgets.cpp
    ${imgui_SOURCE_DIR}/backends/imgui_impl_glfw.cpp
    ${imgui_SOURCE_DIR}/backends/imgui_impl_opengl3.cpp
)
target_include_directories(imgui PUBLIC
    ${imgui_SOURCE_DIR}
    ${imgui_SOURCE_DIR}/backends
)
target_link_libraries(imgui PUBLIC glfw)
if(WIN32)
    target_link_libraries(imgui PUBLIC opengl32)
endif()

# Main executable
add_executable(kufeditor
    src/main.cpp
)
target_link_libraries(kufeditor PRIVATE
    imgui
    glfw
    nlohmann_json::nlohmann_json
)
target_include_directories(kufeditor PRIVATE src)

# Tests
enable_testing()
add_executable(kufeditor_tests
    test/main_test.cpp
)
target_link_libraries(kufeditor_tests PRIVATE Catch2::Catch2WithMain)
include(CTest)
include(Catch)
catch_discover_tests(kufeditor_tests)
```

`src/main.cpp`:
```cpp
int main() {
    return 0;
}
```

`test/main_test.cpp`:
```cpp
#include <catch2/catch_test_macros.hpp>

TEST_CASE("Build system works", "[setup]") {
    REQUIRE(1 + 1 == 2);
}
```

**Verify:**
```bash
mkdir build && cd build
cmake .. -G "Visual Studio 17 2022"
cmake --build . --config Release
ctest -C Release
# Expected: 1 test passed
```

---

### Task 1.2: Create Directory Structure

**Files:**
- Create: `src/core/.gitkeep`
- Create: `src/formats/.gitkeep`
- Create: `src/ui/views/.gitkeep`
- Create: `src/ui/widgets/.gitkeep`
- Create: `src/ui/dialogs/.gitkeep`
- Create: `src/backup/.gitkeep`
- Create: `src/undo/.gitkeep`
- Create: `resources/.gitkeep`
- Create: `test/data/.gitkeep`

**Steps:**
1. Create all directories with .gitkeep placeholder files
2. Commit: "Add project directory structure"

**Verify:**
```bash
ls -la src/
# Expected: core, formats, ui, backup, undo directories
```

---

## Phase 2: Core Application Framework

### Task 2.1: Create Window with GLFW and OpenGL

**Files:**
- Create: `src/core/window.h`
- Create: `src/core/window.cpp`
- Modify: `src/main.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Write test that Window can be created and destroyed
2. Implement Window class with GLFW initialization
3. Update main.cpp to create window
4. Verify window opens and closes cleanly
5. Commit: "Add GLFW window management"

**Code:**

`src/core/window.h`:
```cpp
#pragma once

#include <string>
#include <string_view>

struct GLFWwindow;

namespace kuf {

class Window {
public:
    Window(std::string_view title, int width, int height);
    ~Window();

    Window(const Window&) = delete;
    Window& operator=(const Window&) = delete;

    bool shouldClose() const;
    void pollEvents();
    void swapBuffers();

    GLFWwindow* handle() const { return window_; }
    int width() const { return width_; }
    int height() const { return height_; }

private:
    GLFWwindow* window_ = nullptr;
    int width_;
    int height_;
};

} // namespace kuf
```

`src/core/window.cpp`:
```cpp
#include "core/window.h"

#include <GLFW/glfw3.h>
#include <stdexcept>

namespace kuf {

Window::Window(std::string_view title, int width, int height)
    : width_(width), height_(height) {
    if (!glfwInit()) {
        throw std::runtime_error("Failed to initialize GLFW");
    }

    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    window_ = glfwCreateWindow(width, height, std::string(title).c_str(), nullptr, nullptr);
    if (!window_) {
        glfwTerminate();
        throw std::runtime_error("Failed to create GLFW window");
    }

    glfwMakeContextCurrent(window_);
    glfwSwapInterval(1);
}

Window::~Window() {
    if (window_) {
        glfwDestroyWindow(window_);
    }
    glfwTerminate();
}

bool Window::shouldClose() const {
    return glfwWindowShouldClose(window_);
}

void Window::pollEvents() {
    glfwPollEvents();
}

void Window::swapBuffers() {
    glfwSwapBuffers(window_);
}

} // namespace kuf
```

Update `src/main.cpp`:
```cpp
#include "core/window.h"

int main() {
    kuf::Window window("KUF Editor", 1280, 720);

    while (!window.shouldClose()) {
        window.pollEvents();
        window.swapBuffers();
    }

    return 0;
}
```

Update `CMakeLists.txt` (add to kufeditor sources):
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
)
```

**Verify:**
```bash
cmake --build build --config Release
./build/Release/kufeditor.exe
# Expected: Window opens, can be closed with X button
```

---

### Task 2.2: Initialize Dear ImGui

**Files:**
- Create: `src/core/imgui_context.h`
- Create: `src/core/imgui_context.cpp`
- Modify: `src/main.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Create ImGuiContext class to manage ImGui lifecycle
2. Initialize with docking enabled
3. Render basic ImGui demo window
4. Verify ImGui renders correctly
5. Commit: "Add Dear ImGui initialization with docking"

**Code:**

`src/core/imgui_context.h`:
```cpp
#pragma once

struct GLFWwindow;

namespace kuf {

class ImGuiContext {
public:
    explicit ImGuiContext(GLFWwindow* window);
    ~ImGuiContext();

    ImGuiContext(const ImGuiContext&) = delete;
    ImGuiContext& operator=(const ImGuiContext&) = delete;

    void beginFrame();
    void endFrame();

private:
    void applyDarkTheme();
};

} // namespace kuf
```

`src/core/imgui_context.cpp`:
```cpp
#include "core/imgui_context.h"

#include <imgui.h>
#include <imgui_impl_glfw.h>
#include <imgui_impl_opengl3.h>
#include <GLFW/glfw3.h>

namespace kuf {

ImGuiContext::ImGuiContext(GLFWwindow* window) {
    IMGUI_CHECKVERSION();
    ImGui::CreateContext();

    ImGuiIO& io = ImGui::GetIO();
    io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;

    applyDarkTheme();

    ImGui_ImplGlfw_InitForOpenGL(window, true);
    ImGui_ImplOpenGL3_Init("#version 330");
}

ImGuiContext::~ImGuiContext() {
    ImGui_ImplOpenGL3_Shutdown();
    ImGui_ImplGlfw_Shutdown();
    ImGui::DestroyContext();
}

void ImGuiContext::beginFrame() {
    ImGui_ImplOpenGL3_NewFrame();
    ImGui_ImplGlfw_NewFrame();
    ImGui::NewFrame();
}

void ImGuiContext::endFrame() {
    ImGui::Render();
    ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());
}

void ImGuiContext::applyDarkTheme() {
    ImGuiStyle& style = ImGui::GetStyle();
    ImVec4* colors = style.Colors;

    colors[ImGuiCol_WindowBg] = ImVec4(0.12f, 0.12f, 0.12f, 1.00f);
    colors[ImGuiCol_ChildBg] = ImVec4(0.12f, 0.12f, 0.12f, 0.00f);
    colors[ImGuiCol_PopupBg] = ImVec4(0.15f, 0.15f, 0.15f, 0.94f);
    colors[ImGuiCol_Border] = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_FrameBg] = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_FrameBgHovered] = ImVec4(0.30f, 0.30f, 0.30f, 1.00f);
    colors[ImGuiCol_FrameBgActive] = ImVec4(0.35f, 0.35f, 0.35f, 1.00f);
    colors[ImGuiCol_TitleBg] = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_TitleBgActive] = ImVec4(0.25f, 0.25f, 0.25f, 1.00f);
    colors[ImGuiCol_MenuBarBg] = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_Header] = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_HeaderHovered] = ImVec4(0.31f, 0.31f, 0.31f, 1.00f);
    colors[ImGuiCol_HeaderActive] = ImVec4(0.39f, 0.39f, 0.39f, 1.00f);
    colors[ImGuiCol_Tab] = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_TabHovered] = ImVec4(0.11f, 0.59f, 0.92f, 0.80f);
    colors[ImGuiCol_TabActive] = ImVec4(0.00f, 0.48f, 0.80f, 1.00f);
    colors[ImGuiCol_Text] = ImVec4(0.83f, 0.83f, 0.83f, 1.00f);
    colors[ImGuiCol_TextDisabled] = ImVec4(0.50f, 0.50f, 0.50f, 1.00f);

    style.WindowRounding = 4.0f;
    style.FrameRounding = 2.0f;
    style.ScrollbarRounding = 2.0f;
    style.GrabRounding = 2.0f;
    style.TabRounding = 2.0f;
    style.WindowPadding = ImVec2(8, 8);
    style.FramePadding = ImVec2(6, 4);
    style.ItemSpacing = ImVec2(8, 4);
}

} // namespace kuf
```

Update `src/main.cpp`:
```cpp
#include "core/window.h"
#include "core/imgui_context.h"

#include <GLFW/glfw3.h>
#include <imgui.h>

int main() {
    kuf::Window window("KUF Editor", 1280, 720);
    kuf::ImGuiContext imgui(window.handle());

    while (!window.shouldClose()) {
        window.pollEvents();

        imgui.beginFrame();

        // Show demo window for testing
        ImGui::ShowDemoWindow();

        imgui.endFrame();

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        window.swapBuffers();
    }

    return 0;
}
```

Update `CMakeLists.txt`:
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
)
```

**Verify:**
```bash
cmake --build build --config Release
./build/Release/kufeditor.exe
# Expected: Window with ImGui demo window, dockable panels
```

---

### Task 2.3: Create Main Dockspace Layout

**Files:**
- Create: `src/core/application.h`
- Create: `src/core/application.cpp`
- Modify: `src/main.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Create Application class managing window and ImGui
2. Implement main dockspace with menu bar
3. Add File menu with Exit option
4. Verify docking works
5. Commit: "Add main application with dockspace layout"

**Code:**

`src/core/application.h`:
```cpp
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
```

`src/core/application.cpp`:
```cpp
#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"

#include <imgui.h>
#include <imgui_internal.h>
#include <GLFW/glfw3.h>

namespace kuf {

Application::Application() {
    window_ = std::make_unique<Window>("KUF Editor", 1280, 720);
    imgui_ = std::make_unique<ImGuiContext>(window_->handle());
}

Application::~Application() = default;

void Application::run() {
    while (running_ && !window_->shouldClose()) {
        window_->pollEvents();

        imgui_->beginFrame();

        drawDockspace();

        imgui_->endFrame();

        glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

        window_->swapBuffers();
    }
}

void Application::drawMenuBar() {
    if (ImGui::BeginMainMenuBar()) {
        if (ImGui::BeginMenu("File")) {
            if (ImGui::MenuItem("Open...", "Ctrl+O")) {
                // TODO: Open file dialog
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
            ImGui::MenuItem("File Browser", nullptr, nullptr);
            ImGui::MenuItem("Validation Log", nullptr, nullptr);
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Help")) {
            if (ImGui::MenuItem("About")) {
                // TODO: About dialog
            }
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

    // Status bar
    ImGui::SetCursorPosY(ImGui::GetWindowHeight() - 24);
    ImGui::BeginChild("StatusBar", ImVec2(0, 24), false);
    ImGui::Text("Ready");
    ImGui::EndChild();

    ImGui::End();
}

} // namespace kuf
```

Update `src/main.cpp`:
```cpp
#include "core/application.h"

int main() {
    kuf::Application app;
    app.run();
    return 0;
}
```

Update `CMakeLists.txt`:
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
)
```

**Verify:**
```bash
cmake --build build --config Release
./build/Release/kufeditor.exe
# Expected: Window with menu bar, dockspace, status bar. File > Exit works.
```

---

## Phase 3: File Format Infrastructure

### Task 3.1: Create IFileFormat Interface and Validation Types

**Files:**
- Create: `src/formats/file_format.h`
- Create: `src/formats/validation.h`

**Steps:**
1. Define GameVersion enum
2. Define ValidationIssue struct
3. Define IFileFormat interface
4. Commit: "Add file format interface and validation types"

**Code:**

`src/formats/validation.h`:
```cpp
#pragma once

#include <string>
#include <cstddef>

namespace kuf {

enum class Severity {
    Info,
    Warning,
    Error
};

struct ValidationIssue {
    Severity severity;
    std::string field;
    std::string message;
    size_t recordIndex;
};

} // namespace kuf
```

`src/formats/file_format.h`:
```cpp
#pragma once

#include "formats/validation.h"

#include <cstddef>
#include <span>
#include <string_view>
#include <vector>

namespace kuf {

enum class GameVersion {
    Crusaders,
    Heroes,
    Unknown
};

class IFileFormat {
public:
    virtual ~IFileFormat() = default;

    virtual bool load(std::span<const std::byte> data) = 0;
    virtual std::vector<std::byte> save() const = 0;
    virtual std::string_view formatName() const = 0;
    virtual GameVersion detectedVersion() const = 0;
    virtual std::vector<ValidationIssue> validate() const = 0;
};

} // namespace kuf
```

**Verify:**
```bash
cmake --build build --config Release
# Expected: Compiles without errors
```

---

### Task 3.2: Implement Binary SOX Parser for TroopInfo

**Files:**
- Create: `src/formats/sox_binary.h`
- Create: `src/formats/sox_binary.cpp`
- Create: `test/sox_binary_test.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Write test for loading TroopInfo.sox header
2. Implement SoxBinary class with header parsing
3. Add TroopInfo record structure
4. Implement record iteration
5. Commit: "Add binary SOX parser for TroopInfo"

**Code:**

`src/formats/sox_binary.h`:
```cpp
#pragma once

#include "formats/file_format.h"

#include <array>
#include <cstdint>
#include <vector>

namespace kuf {

struct LevelUpData {
    int32_t skillId;
    float skillPerLevel;
};

struct TroopInfo {
    int32_t job;
    int32_t typeId;
    float moveSpeed;
    float rotateRate;
    float moveAcceleration;
    float moveDeceleration;
    float sightRange;
    float attackRangeMax;
    float attackRangeMin;
    float attackFrontRange;
    float directAttack;
    float indirectAttack;
    float defense;
    float baseWidth;
    float resistMelee;
    float resistRanged;
    float resistFrontal;
    float resistExplosion;
    float resistFire;
    float resistIce;
    float resistLightning;
    float resistHoly;
    float resistCurse;
    float resistPoison;
    float maxUnitSpeedMultiplier;
    float defaultUnitHp;
    int32_t formationRandom;
    int32_t defaultUnitNumX;
    int32_t defaultUnitNumY;
    float unitHpLevelUp;
    std::array<LevelUpData, 3> levelUpData;
    float damageDistribution;
};

class SoxBinary : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "Binary SOX"; }
    GameVersion detectedVersion() const override { return version_; }
    std::vector<ValidationIssue> validate() const override;

    int32_t version() const { return headerVersion_; }
    size_t recordCount() const { return troops_.size(); }
    const std::vector<TroopInfo>& troops() const { return troops_; }
    std::vector<TroopInfo>& troops() { return troops_; }

private:
    int32_t headerVersion_ = 0;
    std::vector<TroopInfo> troops_;
    GameVersion version_ = GameVersion::Unknown;
    std::vector<std::byte> footer_;
};

} // namespace kuf
```

`src/formats/sox_binary.cpp`:
```cpp
#include "formats/sox_binary.h"

#include <cstring>

namespace kuf {

namespace {

template<typename T>
T readLE(const std::byte* data) {
    T value;
    std::memcpy(&value, data, sizeof(T));
    return value;
}

template<typename T>
void writeLE(std::byte* data, T value) {
    std::memcpy(data, &value, sizeof(T));
}

constexpr size_t HEADER_SIZE = 8;
constexpr size_t TROOP_RECORD_SIZE = 148;
constexpr size_t FOOTER_SIZE = 64;

} // namespace

bool SoxBinary::load(std::span<const std::byte> data) {
    if (data.size() < HEADER_SIZE) {
        return false;
    }

    headerVersion_ = readLE<int32_t>(data.data());
    int32_t count = readLE<int32_t>(data.data() + 4);

    if (headerVersion_ != 100) {
        return false;
    }

    size_t expectedSize = HEADER_SIZE + (count * TROOP_RECORD_SIZE) + FOOTER_SIZE;
    if (data.size() < expectedSize) {
        return false;
    }

    troops_.clear();
    troops_.reserve(count);

    const std::byte* ptr = data.data() + HEADER_SIZE;
    for (int32_t i = 0; i < count; ++i) {
        TroopInfo troop{};
        troop.job = readLE<int32_t>(ptr + 0x00);
        troop.typeId = readLE<int32_t>(ptr + 0x04);
        troop.moveSpeed = readLE<float>(ptr + 0x08);
        troop.rotateRate = readLE<float>(ptr + 0x0C);
        troop.moveAcceleration = readLE<float>(ptr + 0x10);
        troop.moveDeceleration = readLE<float>(ptr + 0x14);
        troop.sightRange = readLE<float>(ptr + 0x18);
        troop.attackRangeMax = readLE<float>(ptr + 0x1C);
        troop.attackRangeMin = readLE<float>(ptr + 0x20);
        troop.attackFrontRange = readLE<float>(ptr + 0x24);
        troop.directAttack = readLE<float>(ptr + 0x28);
        troop.indirectAttack = readLE<float>(ptr + 0x2C);
        troop.defense = readLE<float>(ptr + 0x30);
        troop.baseWidth = readLE<float>(ptr + 0x34);
        troop.resistMelee = readLE<float>(ptr + 0x38);
        troop.resistRanged = readLE<float>(ptr + 0x3C);
        troop.resistFrontal = readLE<float>(ptr + 0x40);
        troop.resistExplosion = readLE<float>(ptr + 0x44);
        troop.resistFire = readLE<float>(ptr + 0x48);
        troop.resistIce = readLE<float>(ptr + 0x4C);
        troop.resistLightning = readLE<float>(ptr + 0x50);
        troop.resistHoly = readLE<float>(ptr + 0x54);
        troop.resistCurse = readLE<float>(ptr + 0x58);
        troop.resistPoison = readLE<float>(ptr + 0x5C);
        troop.maxUnitSpeedMultiplier = readLE<float>(ptr + 0x60);
        troop.defaultUnitHp = readLE<float>(ptr + 0x64);
        troop.formationRandom = readLE<int32_t>(ptr + 0x68);
        troop.defaultUnitNumX = readLE<int32_t>(ptr + 0x6C);
        troop.defaultUnitNumY = readLE<int32_t>(ptr + 0x70);
        troop.unitHpLevelUp = readLE<float>(ptr + 0x74);

        for (int j = 0; j < 3; ++j) {
            troop.levelUpData[j].skillId = readLE<int32_t>(ptr + 0x78 + j * 8);
            troop.levelUpData[j].skillPerLevel = readLE<float>(ptr + 0x7C + j * 8);
        }

        troop.damageDistribution = readLE<float>(ptr + 0x90);

        troops_.push_back(troop);
        ptr += TROOP_RECORD_SIZE;
    }

    footer_.assign(ptr, ptr + FOOTER_SIZE);
    version_ = GameVersion::Crusaders;

    return true;
}

std::vector<std::byte> SoxBinary::save() const {
    std::vector<std::byte> data;
    data.resize(HEADER_SIZE + troops_.size() * TROOP_RECORD_SIZE + FOOTER_SIZE);

    std::byte* ptr = data.data();
    writeLE(ptr, headerVersion_);
    writeLE(ptr + 4, static_cast<int32_t>(troops_.size()));
    ptr += HEADER_SIZE;

    for (const auto& troop : troops_) {
        writeLE(ptr + 0x00, troop.job);
        writeLE(ptr + 0x04, troop.typeId);
        writeLE(ptr + 0x08, troop.moveSpeed);
        writeLE(ptr + 0x0C, troop.rotateRate);
        writeLE(ptr + 0x10, troop.moveAcceleration);
        writeLE(ptr + 0x14, troop.moveDeceleration);
        writeLE(ptr + 0x18, troop.sightRange);
        writeLE(ptr + 0x1C, troop.attackRangeMax);
        writeLE(ptr + 0x20, troop.attackRangeMin);
        writeLE(ptr + 0x24, troop.attackFrontRange);
        writeLE(ptr + 0x28, troop.directAttack);
        writeLE(ptr + 0x2C, troop.indirectAttack);
        writeLE(ptr + 0x30, troop.defense);
        writeLE(ptr + 0x34, troop.baseWidth);
        writeLE(ptr + 0x38, troop.resistMelee);
        writeLE(ptr + 0x3C, troop.resistRanged);
        writeLE(ptr + 0x40, troop.resistFrontal);
        writeLE(ptr + 0x44, troop.resistExplosion);
        writeLE(ptr + 0x48, troop.resistFire);
        writeLE(ptr + 0x4C, troop.resistIce);
        writeLE(ptr + 0x50, troop.resistLightning);
        writeLE(ptr + 0x54, troop.resistHoly);
        writeLE(ptr + 0x58, troop.resistCurse);
        writeLE(ptr + 0x5C, troop.resistPoison);
        writeLE(ptr + 0x60, troop.maxUnitSpeedMultiplier);
        writeLE(ptr + 0x64, troop.defaultUnitHp);
        writeLE(ptr + 0x68, troop.formationRandom);
        writeLE(ptr + 0x6C, troop.defaultUnitNumX);
        writeLE(ptr + 0x70, troop.defaultUnitNumY);
        writeLE(ptr + 0x74, troop.unitHpLevelUp);

        for (int j = 0; j < 3; ++j) {
            writeLE(ptr + 0x78 + j * 8, troop.levelUpData[j].skillId);
            writeLE(ptr + 0x7C + j * 8, troop.levelUpData[j].skillPerLevel);
        }

        writeLE(ptr + 0x90, troop.damageDistribution);
        ptr += TROOP_RECORD_SIZE;
    }

    std::memcpy(ptr, footer_.data(), footer_.size());

    return data;
}

std::vector<ValidationIssue> SoxBinary::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < troops_.size(); ++i) {
        const auto& troop = troops_[i];

        auto checkResistance = [&](float value, const char* name) {
            if (value < 0.0f || value > 2.0f) {
                issues.push_back({
                    Severity::Warning,
                    name,
                    "Resistance outside normal range (0.0-2.0)",
                    i
                });
            }
        };

        checkResistance(troop.resistMelee, "resistMelee");
        checkResistance(troop.resistRanged, "resistRanged");
        checkResistance(troop.resistFrontal, "resistFrontal");
        checkResistance(troop.resistExplosion, "resistExplosion");
        checkResistance(troop.resistFire, "resistFire");
        checkResistance(troop.resistIce, "resistIce");
        checkResistance(troop.resistLightning, "resistLightning");
        checkResistance(troop.resistHoly, "resistHoly");
        checkResistance(troop.resistCurse, "resistCurse");
        checkResistance(troop.resistPoison, "resistPoison");

        if (troop.defaultUnitHp <= 0) {
            issues.push_back({
                Severity::Error,
                "defaultUnitHp",
                "HP must be positive",
                i
            });
        }
    }

    return issues;
}

} // namespace kuf
```

`test/sox_binary_test.cpp`:
```cpp
#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_floating_point.hpp>

#include "formats/sox_binary.h"

#include <array>
#include <cstring>

namespace {

std::vector<std::byte> createMinimalTroopInfoSox() {
    std::vector<std::byte> data(8 + 148 + 64, std::byte{0});

    // Header: version=100, count=1
    int32_t version = 100;
    int32_t count = 1;
    std::memcpy(data.data(), &version, 4);
    std::memcpy(data.data() + 4, &count, 4);

    // First troop: set some recognizable values
    float moveSpeed = 100.0f;
    float resistMelee = 1.0f;
    float defaultHp = 50.0f;
    std::memcpy(data.data() + 8 + 0x08, &moveSpeed, 4);
    std::memcpy(data.data() + 8 + 0x38, &resistMelee, 4);
    std::memcpy(data.data() + 8 + 0x64, &defaultHp, 4);

    return data;
}

} // namespace

TEST_CASE("SoxBinary parses header correctly", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    REQUIRE(sox.load(data));
    REQUIRE(sox.version() == 100);
    REQUIRE(sox.recordCount() == 1);
}

TEST_CASE("SoxBinary parses troop fields", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    REQUIRE(sox.load(data));
    REQUIRE(sox.troops().size() == 1);

    const auto& troop = sox.troops()[0];
    REQUIRE_THAT(troop.moveSpeed, Catch::Matchers::WithinAbs(100.0f, 0.001f));
    REQUIRE_THAT(troop.resistMelee, Catch::Matchers::WithinAbs(1.0f, 0.001f));
    REQUIRE_THAT(troop.defaultUnitHp, Catch::Matchers::WithinAbs(50.0f, 0.001f));
}

TEST_CASE("SoxBinary round-trip preserves data", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto original = createMinimalTroopInfoSox();

    REQUIRE(sox.load(original));
    auto saved = sox.save();

    REQUIRE(saved.size() == original.size());
    REQUIRE(std::memcmp(saved.data(), original.data(), saved.size()) == 0);
}

TEST_CASE("SoxBinary validates resistance ranges", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    // Set invalid resistance
    float badResist = 3.0f;
    std::memcpy(data.data() + 8 + 0x38, &badResist, 4);

    REQUIRE(sox.load(data));
    auto issues = sox.validate();

    REQUIRE(!issues.empty());
    REQUIRE(issues[0].severity == kuf::Severity::Warning);
}
```

Update `CMakeLists.txt`:
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
    src/formats/sox_binary.cpp
)

add_executable(kufeditor_tests
    test/main_test.cpp
    test/sox_binary_test.cpp
    src/formats/sox_binary.cpp
)
target_link_libraries(kufeditor_tests PRIVATE Catch2::Catch2WithMain)
target_include_directories(kufeditor_tests PRIVATE src)
```

**Verify:**
```bash
cmake --build build --config Release
ctest -C Release --output-on-failure
# Expected: All tests pass
```

---

## Phase 4: Editor UI

### Task 4.1: Create Base View Class

**Files:**
- Create: `src/ui/views/view.h`

**Steps:**
1. Define abstract View base class
2. Include window flags and lifecycle hooks
3. Commit: "Add base View class for editor panels"

**Code:**

`src/ui/views/view.h`:
```cpp
#pragma once

#include <imgui.h>
#include <string>

namespace kuf {

class View {
public:
    View(std::string name) : name_(std::move(name)) {}
    virtual ~View() = default;

    virtual void drawContent() = 0;

    void draw() {
        if (!open_) return;

        ImGui::SetNextWindowSizeConstraints(minSize(), maxSize());

        if (ImGui::Begin(name_.c_str(), &open_, flags())) {
            drawContent();
        }
        ImGui::End();
    }

    bool& isOpen() { return open_; }
    const std::string& name() const { return name_; }

protected:
    virtual ImGuiWindowFlags flags() const { return ImGuiWindowFlags_None; }
    virtual ImVec2 minSize() const { return {200, 100}; }
    virtual ImVec2 maxSize() const { return {FLT_MAX, FLT_MAX}; }

    std::string name_;
    bool open_ = true;
};

} // namespace kuf
```

**Verify:**
```bash
cmake --build build --config Release
# Expected: Compiles without errors
```

---

### Task 4.2: Create Troop Editor View

**Files:**
- Create: `src/ui/views/troop_editor.h`
- Create: `src/ui/views/troop_editor.cpp`
- Modify: `src/core/application.h`
- Modify: `src/core/application.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Create TroopEditorView showing troop table
2. Display troop name, stats, resistances in columns
3. Integrate with Application
4. Commit: "Add TroopEditorView with table display"

**Code:**

`src/ui/views/troop_editor.h`:
```cpp
#pragma once

#include "ui/views/view.h"
#include "formats/sox_binary.h"

#include <memory>

namespace kuf {

class TroopEditorView : public View {
public:
    TroopEditorView();

    void drawContent() override;

    void setData(std::shared_ptr<SoxBinary> data);
    bool hasData() const { return data_ != nullptr; }

private:
    void drawTroopTable();
    void drawTroopDetails(size_t index);

    std::shared_ptr<SoxBinary> data_;
    int selectedTroop_ = -1;

    static constexpr const char* TROOP_NAMES[] = {
        "Archer", "Longbows", "Infantry", "Spearman", "Heavy Infantry",
        "Knight", "Paladin", "Cavalry", "Heavy Cavalry", "Storm Riders",
        "Sappers", "Pyro Techs", "Bomber Wings", "Mortar", "Ballista",
        "Harpoon", "Catapult", "Battaloon", "Dark Elves Archer",
        "Dark Elves Cavalry Archers", "Dark Elves Infantry", "Dark Elves Knights",
        "Dark Elves Cavalry", "Orc Infantry", "Orc Riders", "Orc Heavy Riders",
        "Orc Axe Man", "Orc Heavy Infantry", "Orc Sappers", "Orc Scorpion",
        "Orc Swamp Mammoth", "Orc Dirigible", "Orc Black Wyverns", "Orc Ghouls",
        "Orc Bone Dragon", "Wall Archers (Humans)", "Scouts", "Ghoul Selfdestruct",
        "Encablossa Monster (Melee)", "Encablossa Flying Monster",
        "Encablossa Monster (Ranged)", "Wall Archers (Elves)", "Encablossa Main"
    };
};

} // namespace kuf
```

`src/ui/views/troop_editor.cpp`:
```cpp
#include "ui/views/troop_editor.h"

#include <imgui.h>

namespace kuf {

TroopEditorView::TroopEditorView() : View("Troop Editor") {}

void TroopEditorView::setData(std::shared_ptr<SoxBinary> data) {
    data_ = std::move(data);
    selectedTroop_ = -1;
}

void TroopEditorView::drawContent() {
    if (!data_) {
        ImGui::TextDisabled("No file loaded. Use File > Open to load TroopInfo.sox");
        return;
    }

    ImGui::BeginChild("TroopList", ImVec2(250, 0), true);
    drawTroopTable();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("TroopDetails", ImVec2(0, 0), true);
    if (selectedTroop_ >= 0 && selectedTroop_ < static_cast<int>(data_->troops().size())) {
        drawTroopDetails(selectedTroop_);
    } else {
        ImGui::TextDisabled("Select a troop to edit");
    }
    ImGui::EndChild();
}

void TroopEditorView::drawTroopTable() {
    for (size_t i = 0; i < data_->troops().size(); ++i) {
        const char* name = (i < std::size(TROOP_NAMES)) ? TROOP_NAMES[i] : "Unknown";
        bool selected = (selectedTroop_ == static_cast<int>(i));

        if (ImGui::Selectable(name, selected)) {
            selectedTroop_ = static_cast<int>(i);
        }
    }
}

void TroopEditorView::drawTroopDetails(size_t index) {
    auto& troop = data_->troops()[index];
    const char* name = (index < std::size(TROOP_NAMES)) ? TROOP_NAMES[index] : "Unknown";

    ImGui::Text("%s", name);
    ImGui::Separator();

    if (ImGui::CollapsingHeader("Movement", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Move Speed", &troop.moveSpeed, 1.0f, 0.0f, 10000.0f);
        ImGui::DragFloat("Rotate Rate", &troop.rotateRate, 0.01f, 0.0f, 10.0f);
        ImGui::DragFloat("Acceleration", &troop.moveAcceleration, 1.0f, 0.0f, 1000.0f);
        ImGui::DragFloat("Deceleration", &troop.moveDeceleration, 1.0f, 0.0f, 1000.0f);
    }

    if (ImGui::CollapsingHeader("Combat", ImGuiTreeNodeFlags_DefaultOpen)) {
        ImGui::DragFloat("Sight Range", &troop.sightRange, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Attack Range Max", &troop.attackRangeMax, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Attack Range Min", &troop.attackRangeMin, 10.0f, 0.0f, 50000.0f);
        ImGui::DragFloat("Direct Attack", &troop.directAttack, 0.1f, 0.0f, 100.0f);
        ImGui::DragFloat("Indirect Attack", &troop.indirectAttack, 0.1f, 0.0f, 100.0f);
        ImGui::DragFloat("Defense", &troop.defense, 0.1f, 0.0f, 100.0f);
    }

    if (ImGui::CollapsingHeader("Resistances", ImGuiTreeNodeFlags_DefaultOpen)) {
        auto resistSlider = [](const char* label, float* value) {
            ImGui::SliderFloat(label, value, 0.0f, 2.0f, "%.2f");
            ImGui::SameLine();
            int pct = static_cast<int>((1.0f - *value) * 100.0f);
            ImGui::Text("(%+d%%)", pct);
        };

        resistSlider("Melee", &troop.resistMelee);
        resistSlider("Ranged", &troop.resistRanged);
        resistSlider("Frontal", &troop.resistFrontal);
        resistSlider("Explosion", &troop.resistExplosion);
        resistSlider("Fire", &troop.resistFire);
        resistSlider("Ice", &troop.resistIce);
        resistSlider("Lightning", &troop.resistLightning);
        resistSlider("Holy", &troop.resistHoly);
        resistSlider("Curse", &troop.resistCurse);
        resistSlider("Poison", &troop.resistPoison);
    }

    if (ImGui::CollapsingHeader("Unit Configuration")) {
        ImGui::DragFloat("Default HP", &troop.defaultUnitHp, 1.0f, 1.0f, 10000.0f);
        ImGui::DragInt("Units X", &troop.defaultUnitNumX, 1, 1, 20);
        ImGui::DragInt("Units Y", &troop.defaultUnitNumY, 1, 1, 20);
        ImGui::Text("Total Units: %d", troop.defaultUnitNumX * troop.defaultUnitNumY);
    }
}

} // namespace kuf
```

Update `src/core/application.h`:
```cpp
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
```

Update `src/core/application.cpp`:
```cpp
#include "core/application.h"
#include "core/window.h"
#include "core/imgui_context.h"
#include "ui/views/troop_editor.h"
#include "formats/sox_binary.h"

#include <imgui.h>
#include <imgui_internal.h>
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
                // TODO: Native file dialog - for now hardcode test path
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

    // Status bar
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
```

Update `CMakeLists.txt`:
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
    src/formats/sox_binary.cpp
    src/ui/views/troop_editor.cpp
)
```

**Verify:**
```bash
cmake --build build --config Release
./build/Release/kufeditor.exe
# Expected: Window with Troop Editor panel showing "No file loaded" message
```

---

## Phase 5: Native File Dialog

### Task 5.1: Add Windows File Dialog

**Files:**
- Create: `src/ui/dialogs/file_dialog.h`
- Create: `src/ui/dialogs/file_dialog.cpp`
- Modify: `src/core/application.cpp`
- Modify: `CMakeLists.txt`

**Steps:**
1. Create FileDialog wrapper using Windows API
2. Integrate with File > Open menu
3. Verify file selection works
4. Commit: "Add native Windows file dialog"

**Code:**

`src/ui/dialogs/file_dialog.h`:
```cpp
#pragma once

#include <optional>
#include <string>

namespace kuf {

class FileDialog {
public:
    static std::optional<std::string> openFile(const char* filter);
    static std::optional<std::string> saveFile(const char* filter, const char* defaultName = nullptr);
};

} // namespace kuf
```

`src/ui/dialogs/file_dialog.cpp`:
```cpp
#include "ui/dialogs/file_dialog.h"

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commdlg.h>
#endif

namespace kuf {

std::optional<std::string> FileDialog::openFile(const char* filter) {
#ifdef _WIN32
    char filename[MAX_PATH] = "";

    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;

    if (GetOpenFileNameA(&ofn)) {
        return std::string(filename);
    }
#endif
    return std::nullopt;
}

std::optional<std::string> FileDialog::saveFile(const char* filter, const char* defaultName) {
#ifdef _WIN32
    char filename[MAX_PATH] = "";
    if (defaultName) {
        strncpy_s(filename, defaultName, MAX_PATH - 1);
    }

    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;

    if (GetSaveFileNameA(&ofn)) {
        return std::string(filename);
    }
#endif
    return std::nullopt;
}

} // namespace kuf
```

Update File > Open in `src/core/application.cpp`:
```cpp
#include "ui/dialogs/file_dialog.h"

// In drawMenuBar():
if (ImGui::MenuItem("Open...", "Ctrl+O")) {
    if (auto path = FileDialog::openFile("SOX Files\0*.sox\0All Files\0*.*\0")) {
        openFile(*path);
    }
}
```

Update `CMakeLists.txt`:
```cmake
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
    src/formats/sox_binary.cpp
    src/ui/views/troop_editor.cpp
    src/ui/dialogs/file_dialog.cpp
)

if(WIN32)
    target_link_libraries(kufeditor PRIVATE comdlg32)
endif()
```

**Verify:**
```bash
cmake --build build --config Release
./build/Release/kufeditor.exe
# Expected: File > Open shows Windows file picker
# Expected: Selecting TroopInfo.sox loads and displays troop list
```

---

## Remaining Phases (Summary)

The implementation plan continues with these phases. Each follows the same detailed format:

### Phase 6: Undo/Redo System
- Task 6.1: Create ICommand interface
- Task 6.2: Implement UndoStack
- Task 6.3: Create SetFieldCommand template
- Task 6.4: Integrate with TroopEditorView

### Phase 7: BackupManager
- Task 7.1: Create BackupManager class
- Task 7.2: Implement first-run dialog
- Task 7.3: Add restore functionality
- Task 7.4: Integrate with Application

### Phase 8: Validation Log View
- Task 8.1: Create ValidationLogView
- Task 8.2: Display validation issues
- Task 8.3: Click-to-navigate to issues

### Phase 9: Mission File Parser
- Task 9.1: Create Mission struct definitions
- Task 9.2: Implement MissionFile parser
- Task 9.3: Create MissionEditorView

### Phase 10: Text SOX Parser
- Task 10.1: Implement SoxText parser
- Task 10.2: Create TextSoxEditorView

### Phase 11: Save Game Parser
- Task 11.1: Implement SaveGame parser
- Task 11.2: Create SaveEditorView

### Phase 12: Polish
- Task 12.1: Add keyboard shortcuts
- Task 12.2: Implement dirty flag and save confirmation
- Task 12.3: Add recent files menu
- Task 12.4: Final testing and bug fixes

---

## Execution

To execute this plan:

1. **Subagent-Driven**: Dispatch fresh subagents per task in current session
2. **Manual**: Work through tasks sequentially, committing after each

Each task is 2-5 minutes. Total estimated: ~40-50 tasks across all phases.
