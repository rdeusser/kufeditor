# ImHex-Style UI Recreation Guide

A comprehensive guide for recreating the look, feel, and behavior of ImHex's professional desktop application UI in any C++ application.

---

## Table of Contents

1. [Technology Stack](#1-technology-stack)
2. [Application Architecture](#2-application-architecture)
3. [Window and Rendering Setup](#3-window-and-rendering-setup)
4. [Theming System](#4-theming-system)
5. [Layout and Docking](#5-layout-and-docking)
6. [Custom Widgets](#6-custom-widgets)
7. [Typography and Icons](#7-typography-and-icons)
8. [Interaction Patterns](#8-interaction-patterns)
9. [Event System](#9-event-system)
10. [Best Practices](#10-best-practices)

---

## 1. Technology Stack

### Core Libraries

| Library | Purpose | Version |
|---------|---------|---------|
| **Dear ImGui** | Immediate-mode GUI framework | Latest with docking branch |
| **GLFW 3** | Cross-platform window management | 3.3+ |
| **OpenGL** | GPU rendering backend | 3.0+ (4.1 on macOS) |
| **FreeType** | Font rasterization | 2.x |
| **stb_image** | Image loading (PNG, JPEG, BMP) | Header-only |
| **LunaSVG** | Vector graphics rendering | Latest |

### ImGui Extensions

- **ImPlot** - 2D plotting and charts
- **ImPlot3D** - 3D visualization
- **ImNodes** - Node-based graph editors

### Minimal CMake Setup

```cmake
cmake_minimum_required(VERSION 3.16)
project(MyApp CXX)
set(CMAKE_CXX_STANDARD 20)

find_package(glfw3 REQUIRED)
find_package(OpenGL REQUIRED)
find_package(Freetype REQUIRED)

# ImGui with docking support
add_subdirectory(external/imgui)

target_link_libraries(${PROJECT_NAME}
    glfw
    OpenGL::GL
    Freetype::Freetype
    imgui
)
```

---

## 2. Application Architecture

### Layer Structure

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                     │
│  (Business logic, data models, file handling)           │
├─────────────────────────────────────────────────────────┤
│                      View Layer                          │
│  (Views, Popups, Toasts, Banners)                       │
├─────────────────────────────────────────────────────────┤
│                    Widget Layer                          │
│  (Custom ImGui widgets, ImGuiExt namespace)             │
├─────────────────────────────────────────────────────────┤
│                   ImGui + Extensions                     │
│  (ImGui, ImPlot, ImNodes)                               │
├─────────────────────────────────────────────────────────┤
│                   Platform Layer                         │
│  (GLFW, OpenGL, platform-specific code)                 │
└─────────────────────────────────────────────────────────┘
```

### View System Pattern

Create a base `View` class that all UI panels inherit from:

```cpp
class View {
public:
    enum class Type { Window, Floating, Modal, Special };

    View(std::string name, const char* icon)
        : m_name(std::move(name)), m_icon(icon) {}
    virtual ~View() = default;

    // Override these in derived classes
    virtual void drawContent() = 0;
    virtual void drawAlwaysVisibleContent() {}

    // Lifecycle hooks
    virtual void onOpen() {}
    virtual void onClose() {}

    // Window configuration
    virtual ImGuiWindowFlags getFlags() const { return ImGuiWindowFlags_None; }
    virtual ImVec2 getMinSize() const { return {0, 0}; }
    virtual ImVec2 getMaxSize() const { return {FLT_MAX, FLT_MAX}; }
    virtual bool shouldDraw() const { return true; }

    // Final draw method - handles window creation
    void draw() {
        if (!shouldDraw()) return;

        ImGui::SetNextWindowSizeConstraints(getMinSize(), getMaxSize());

        bool wasOpen = m_open;
        if (ImGui::Begin(m_name.c_str(), &m_open, getFlags())) {
            if (!wasOpen && m_open) onOpen();
            drawContent();
        }
        ImGui::End();

        if (wasOpen && !m_open) onClose();
        drawAlwaysVisibleContent();
    }

    bool& getOpenState() { return m_open; }
    const std::string& getName() const { return m_name; }

protected:
    std::string m_name;
    const char* m_icon;
    bool m_open = true;
};
```

---

## 3. Window and Rendering Setup

### Initialization Sequence

```cpp
class Window {
public:
    bool init() {
        // 1. Initialize GLFW
        if (!glfwInit()) return false;

        // Set OpenGL version hints
        #ifdef __APPLE__
            glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
            glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 1);
            glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
            glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GL_TRUE);
        #else
            glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
            glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 0);
        #endif

        // 2. Create window
        m_window = glfwCreateWindow(1280, 720, "My Application", nullptr, nullptr);
        if (!m_window) return false;

        glfwMakeContextCurrent(m_window);
        glfwSwapInterval(1); // VSync

        // 3. Initialize ImGui
        IMGUI_CHECKVERSION();
        ImGui::CreateContext();
        ImPlot::CreateContext();    // If using ImPlot
        ImNodes::CreateContext();   // If using ImNodes

        ImGuiIO& io = ImGui::GetIO();
        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
        io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable;
        io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;

        // 4. Initialize platform/renderer backends
        ImGui_ImplGlfw_InitForOpenGL(m_window, true);
        ImGui_ImplOpenGL3_Init(getGlslVersion());

        // 5. Load fonts and apply theme
        loadFonts();
        applyTheme();

        return true;
    }

    void mainLoop() {
        while (!glfwWindowShouldClose(m_window)) {
            glfwPollEvents();

            // Start ImGui frame
            ImGui_ImplOpenGL3_NewFrame();
            ImGui_ImplGlfw_NewFrame();
            ImGui::NewFrame();

            // Draw UI
            drawFrame();

            // Render
            ImGui::Render();
            int display_w, display_h;
            glfwGetFramebufferSize(m_window, &display_w, &display_h);
            glViewport(0, 0, display_w, display_h);
            glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT);
            ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

            // Handle viewports
            if (ImGui::GetIO().ConfigFlags & ImGuiConfigFlags_ViewportsEnable) {
                GLFWwindow* backup = glfwGetCurrentContext();
                ImGui::UpdatePlatformWindows();
                ImGui::RenderPlatformWindowsDefault();
                glfwMakeContextCurrent(backup);
            }

            glfwSwapBuffers(m_window);
        }
    }

private:
    GLFWwindow* m_window = nullptr;

    const char* getGlslVersion() {
        #ifdef __APPLE__
            return "#version 410";
        #else
            return "#version 130";
        #endif
    }
};
```

### Frame Rate Control

```cpp
// Adaptive frame rate for power saving
class FrameRateController {
public:
    void beginFrame() {
        m_frameStart = std::chrono::high_resolution_clock::now();
    }

    void endFrame() {
        auto elapsed = std::chrono::high_resolution_clock::now() - m_frameStart;
        auto targetTime = std::chrono::milliseconds(1000 / m_targetFps);

        if (elapsed < targetTime) {
            std::this_thread::sleep_for(targetTime - elapsed);
        }
    }

    void setTargetFps(int fps) { m_targetFps = fps; }

    // Lower FPS when window is not focused
    void setIdleMode(bool idle) {
        m_targetFps = idle ? 5 : 60;
    }

private:
    int m_targetFps = 60;
    std::chrono::high_resolution_clock::time_point m_frameStart;
};
```

---

## 4. Theming System

### Color Scheme Architecture

Define custom colors beyond ImGui's defaults:

```cpp
enum ImGuiCustomCol_ {
    // Toolbar button colors
    ImGuiCustomCol_ToolbarGray,
    ImGuiCustomCol_ToolbarRed,
    ImGuiCustomCol_ToolbarYellow,
    ImGuiCustomCol_ToolbarGreen,
    ImGuiCustomCol_ToolbarBlue,
    ImGuiCustomCol_ToolbarPurple,
    ImGuiCustomCol_ToolbarBrown,

    // Status/log colors
    ImGuiCustomCol_LogDebug,
    ImGuiCustomCol_LogInfo,
    ImGuiCustomCol_LogWarning,
    ImGuiCustomCol_LogError,

    // Diff colors
    ImGuiCustomCol_DiffAdded,
    ImGuiCustomCol_DiffRemoved,
    ImGuiCustomCol_DiffChanged,

    // Special UI elements
    ImGuiCustomCol_Highlight,
    ImGuiCustomCol_Selection,

    ImGuiCustomCol_COUNT
};

struct CustomStyleData {
    ImVec4 Colors[ImGuiCustomCol_COUNT];
    float WindowBlur = 0.0f;
    float PopupAlpha = 0.65f;
};

static CustomStyleData s_customStyle;

ImU32 GetCustomColorU32(ImGuiCustomCol idx, float alpha = 1.0f) {
    ImVec4 c = s_customStyle.Colors[idx];
    c.w *= alpha;
    return ImGui::ColorConvertFloat4ToU32(c);
}
```

### Theme Definition (JSON-based)

```json
{
    "name": "Dark",
    "colors": {
        "imgui": {
            "WindowBg": "#1E1E1EFF",
            "ChildBg": "#1E1E1E00",
            "PopupBg": "#252526F0",
            "Border": "#3C3C3CFF",
            "FrameBg": "#3C3C3CFF",
            "FrameBgHovered": "#4D4D4DFF",
            "FrameBgActive": "#5A5A5AFF",
            "TitleBg": "#2D2D30FF",
            "TitleBgActive": "#3F3F41FF",
            "MenuBarBg": "#2D2D30FF",
            "ScrollbarBg": "#1E1E1EFF",
            "ScrollbarGrab": "#5A5A5AFF",
            "Button": "#3C3C3CFF",
            "ButtonHovered": "#505050FF",
            "ButtonActive": "#646464FF",
            "Header": "#3C3C3CFF",
            "HeaderHovered": "#505050FF",
            "HeaderActive": "#646464FF",
            "Tab": "#2D2D30FF",
            "TabHovered": "#1C97EAFF",
            "TabActive": "#007ACCFF",
            "Text": "#D4D4D4FF",
            "TextDisabled": "#808080FF"
        },
        "custom": {
            "ToolbarGray": "#808080FF",
            "ToolbarRed": "#F14C4CFF",
            "ToolbarYellow": "#CCA700FF",
            "ToolbarGreen": "#89D185FF",
            "ToolbarBlue": "#3794FFFF",
            "ToolbarPurple": "#C586C0FF",
            "LogDebug": "#808080FF",
            "LogInfo": "#3794FFFF",
            "LogWarning": "#CCA700FF",
            "LogError": "#F14C4CFF"
        }
    },
    "styles": {
        "WindowRounding": 4.0,
        "FrameRounding": 2.0,
        "ScrollbarRounding": 2.0,
        "GrabRounding": 2.0,
        "TabRounding": 2.0,
        "WindowPadding": [8, 8],
        "FramePadding": [6, 4],
        "ItemSpacing": [8, 4],
        "ScrollbarSize": 14
    }
}
```

### Theme Application

```cpp
void applyDarkTheme() {
    ImGuiStyle& style = ImGui::GetStyle();
    ImVec4* colors = style.Colors;

    // Window
    colors[ImGuiCol_WindowBg]           = ImVec4(0.12f, 0.12f, 0.12f, 1.00f);
    colors[ImGuiCol_ChildBg]            = ImVec4(0.12f, 0.12f, 0.12f, 0.00f);
    colors[ImGuiCol_PopupBg]            = ImVec4(0.15f, 0.15f, 0.15f, 0.94f);

    // Borders
    colors[ImGuiCol_Border]             = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_BorderShadow]       = ImVec4(0.00f, 0.00f, 0.00f, 0.00f);

    // Frame backgrounds
    colors[ImGuiCol_FrameBg]            = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_FrameBgHovered]     = ImVec4(0.30f, 0.30f, 0.30f, 1.00f);
    colors[ImGuiCol_FrameBgActive]      = ImVec4(0.35f, 0.35f, 0.35f, 1.00f);

    // Title bar
    colors[ImGuiCol_TitleBg]            = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_TitleBgActive]      = ImVec4(0.25f, 0.25f, 0.25f, 1.00f);
    colors[ImGuiCol_TitleBgCollapsed]   = ImVec4(0.18f, 0.18f, 0.18f, 0.75f);

    // Menu bar
    colors[ImGuiCol_MenuBarBg]          = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);

    // Scrollbar
    colors[ImGuiCol_ScrollbarBg]        = ImVec4(0.12f, 0.12f, 0.12f, 1.00f);
    colors[ImGuiCol_ScrollbarGrab]      = ImVec4(0.35f, 0.35f, 0.35f, 1.00f);
    colors[ImGuiCol_ScrollbarGrabHovered] = ImVec4(0.45f, 0.45f, 0.45f, 1.00f);
    colors[ImGuiCol_ScrollbarGrabActive]  = ImVec4(0.55f, 0.55f, 0.55f, 1.00f);

    // Buttons
    colors[ImGuiCol_Button]             = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_ButtonHovered]      = ImVec4(0.31f, 0.31f, 0.31f, 1.00f);
    colors[ImGuiCol_ButtonActive]       = ImVec4(0.39f, 0.39f, 0.39f, 1.00f);

    // Headers (collapsing headers, tree nodes, selectables)
    colors[ImGuiCol_Header]             = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_HeaderHovered]      = ImVec4(0.31f, 0.31f, 0.31f, 1.00f);
    colors[ImGuiCol_HeaderActive]       = ImVec4(0.39f, 0.39f, 0.39f, 1.00f);

    // Tabs
    colors[ImGuiCol_Tab]                = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_TabHovered]         = ImVec4(0.11f, 0.59f, 0.92f, 0.80f);
    colors[ImGuiCol_TabActive]          = ImVec4(0.00f, 0.48f, 0.80f, 1.00f);
    colors[ImGuiCol_TabUnfocused]       = ImVec4(0.18f, 0.18f, 0.18f, 1.00f);
    colors[ImGuiCol_TabUnfocusedActive] = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);

    // Text
    colors[ImGuiCol_Text]               = ImVec4(0.83f, 0.83f, 0.83f, 1.00f);
    colors[ImGuiCol_TextDisabled]       = ImVec4(0.50f, 0.50f, 0.50f, 1.00f);

    // Separators
    colors[ImGuiCol_Separator]          = ImVec4(0.24f, 0.24f, 0.24f, 1.00f);
    colors[ImGuiCol_SeparatorHovered]   = ImVec4(0.31f, 0.31f, 0.31f, 1.00f);
    colors[ImGuiCol_SeparatorActive]    = ImVec4(0.39f, 0.39f, 0.39f, 1.00f);

    // Resize grip
    colors[ImGuiCol_ResizeGrip]         = ImVec4(0.24f, 0.24f, 0.24f, 0.25f);
    colors[ImGuiCol_ResizeGripHovered]  = ImVec4(0.31f, 0.31f, 0.31f, 0.67f);
    colors[ImGuiCol_ResizeGripActive]   = ImVec4(0.39f, 0.39f, 0.39f, 0.95f);

    // Docking
    colors[ImGuiCol_DockingPreview]     = ImVec4(0.00f, 0.48f, 0.80f, 0.70f);
    colors[ImGuiCol_DockingEmptyBg]     = ImVec4(0.12f, 0.12f, 0.12f, 1.00f);

    // Style properties
    style.WindowRounding    = 4.0f;
    style.ChildRounding     = 4.0f;
    style.FrameRounding     = 2.0f;
    style.PopupRounding     = 4.0f;
    style.ScrollbarRounding = 2.0f;
    style.GrabRounding      = 2.0f;
    style.TabRounding       = 2.0f;

    style.WindowPadding     = ImVec2(8, 8);
    style.FramePadding      = ImVec2(6, 4);
    style.ItemSpacing       = ImVec2(8, 4);
    style.ItemInnerSpacing  = ImVec2(4, 4);
    style.IndentSpacing     = 20.0f;
    style.ScrollbarSize     = 14.0f;
    style.GrabMinSize       = 10.0f;

    style.WindowBorderSize  = 1.0f;
    style.ChildBorderSize   = 1.0f;
    style.PopupBorderSize   = 1.0f;
    style.FrameBorderSize   = 0.0f;
    style.TabBorderSize     = 0.0f;

    // Custom colors
    s_customStyle.Colors[ImGuiCustomCol_ToolbarGray]   = ImVec4(0.50f, 0.50f, 0.50f, 1.00f);
    s_customStyle.Colors[ImGuiCustomCol_ToolbarRed]    = ImVec4(0.95f, 0.30f, 0.30f, 1.00f);
    s_customStyle.Colors[ImGuiCustomCol_ToolbarYellow] = ImVec4(0.80f, 0.65f, 0.00f, 1.00f);
    s_customStyle.Colors[ImGuiCustomCol_ToolbarGreen]  = ImVec4(0.54f, 0.82f, 0.52f, 1.00f);
    s_customStyle.Colors[ImGuiCustomCol_ToolbarBlue]   = ImVec4(0.22f, 0.58f, 1.00f, 1.00f);
    s_customStyle.Colors[ImGuiCustomCol_ToolbarPurple] = ImVec4(0.77f, 0.52f, 0.75f, 1.00f);
}
```

### DPI Scaling

```cpp
float getGlobalScale() {
    float xscale, yscale;
    glfwGetWindowContentScale(m_window, &xscale, &yscale);
    return xscale;  // Usually xscale == yscale
}

// User-defined literal for scaled values
float operator""_scaled(long double value) {
    return static_cast<float>(value) * getGlobalScale();
}

// Usage: 8.0_scaled returns 8.0 * DPI scale
```

---

## 5. Layout and Docking

### Main Dockspace Setup

```cpp
void drawMainDockspace() {
    // Make the main window fill the entire screen
    ImGuiViewport* viewport = ImGui::GetMainViewport();
    ImGui::SetNextWindowPos(viewport->WorkPos);
    ImGui::SetNextWindowSize(viewport->WorkSize);
    ImGui::SetNextWindowViewport(viewport->ID);

    ImGuiWindowFlags windowFlags =
        ImGuiWindowFlags_NoDocking |
        ImGuiWindowFlags_NoTitleBar |
        ImGuiWindowFlags_NoCollapse |
        ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove |
        ImGuiWindowFlags_NoBringToFrontOnFocus |
        ImGuiWindowFlags_NoNavFocus |
        ImGuiWindowFlags_NoBackground;

    ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));

    ImGui::Begin("MainDockspaceWindow", nullptr, windowFlags);
    ImGui::PopStyleVar(3);

    // Create the dockspace
    ImGuiID dockspaceId = ImGui::GetID("MainDockspace");
    ImGui::DockSpace(dockspaceId, ImVec2(0, 0), ImGuiDockNodeFlags_PassthruCentralNode);

    ImGui::End();
}
```

### Window Layout Structure

```
┌──────────────────────────────────────────────────────────┐
│  Menu Bar                                                 │
├─────────┬────────────────────────────────────────────────┤
│         │                                                 │
│ Sidebar │              Main Dock Space                   │
│ (icons) │    ┌────────────────┬──────────────────┐       │
│         │    │   View A       │    View B        │       │
│         │    │   (tabbed)     │                  │       │
│         │    ├────────────────┴──────────────────┤       │
│         │    │          View C                   │       │
│         │    │         (bottom)                  │       │
│         │    └───────────────────────────────────┘       │
├─────────┴────────────────────────────────────────────────┤
│  Status Bar / Footer                                      │
└──────────────────────────────────────────────────────────┘
```

### Layout Persistence

```cpp
class LayoutManager {
public:
    void saveLayout(const std::string& name) {
        size_t size;
        const char* data = ImGui::SaveIniSettingsToMemory(&size);

        // Save to file
        std::ofstream file(getLayoutPath(name));
        file.write(data, size);
    }

    void loadLayout(const std::string& name) {
        std::ifstream file(getLayoutPath(name));
        std::string content((std::istreambuf_iterator<char>(file)),
                             std::istreambuf_iterator<char>());

        ImGui::LoadIniSettingsFromMemory(content.c_str(), content.size());
    }

    void lockLayout(bool locked) {
        m_locked = locked;
    }

    bool isLocked() const { return m_locked; }

private:
    bool m_locked = false;

    std::string getLayoutPath(const std::string& name) {
        return "layouts/" + name + ".ini";
    }
};
```

### Workspace System

```cpp
struct Workspace {
    std::string name;
    std::string layout;  // Serialized ImGui layout
    bool builtin;
};

class WorkspaceManager {
public:
    void createWorkspace(const std::string& name) {
        size_t size;
        const char* data = ImGui::SaveIniSettingsToMemory(&size);

        Workspace ws;
        ws.name = name;
        ws.layout = std::string(data, size);
        ws.builtin = false;

        m_workspaces[name] = ws;
    }

    void switchWorkspace(const std::string& name) {
        if (auto it = m_workspaces.find(name); it != m_workspaces.end()) {
            // Close all views first
            for (auto& view : m_views) {
                view->getOpenState() = false;
            }

            // Load the workspace layout
            ImGui::LoadIniSettingsFromMemory(
                it->second.layout.c_str(),
                it->second.layout.size()
            );
        }
    }

private:
    std::map<std::string, Workspace> m_workspaces;
    std::vector<View*> m_views;
};
```

---

## 6. Custom Widgets

### Text Variants

```cpp
namespace ImGuiExt {

// Clickable hyperlink
bool Hyperlink(const char* label) {
    ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(0.22f, 0.58f, 1.00f, 1.00f));
    ImGui::TextUnformatted(label);
    ImGui::PopStyleColor();

    if (ImGui::IsItemHovered()) {
        ImGui::SetMouseCursor(ImGuiMouseCursor_Hand);
        // Draw underline
        ImVec2 min = ImGui::GetItemRectMin();
        ImVec2 max = ImGui::GetItemRectMax();
        ImGui::GetWindowDrawList()->AddLine(
            ImVec2(min.x, max.y),
            ImVec2(max.x, max.y),
            ImGui::GetColorU32(ImGuiCol_Text)
        );
    }

    return ImGui::IsItemClicked();
}

// Section header with separator
void Header(const char* label, bool firstEntry = false) {
    if (!firstEntry) {
        ImGui::NewLine();
    }
    ImGui::TextUnformatted(label);
    ImGui::Separator();
}

// Text with colored background highlight
void TextColored(const char* label, ImU32 color) {
    ImVec2 pos = ImGui::GetCursorScreenPos();
    ImVec2 textSize = ImGui::CalcTextSize(label);

    ImGui::GetWindowDrawList()->AddRectFilled(
        pos,
        ImVec2(pos.x + textSize.x, pos.y + textSize.y),
        color,
        2.0f
    );

    ImGui::TextUnformatted(label);
}

// Spinning text indicator
void TextSpinner(const char* label) {
    static const char* spinChars = "|/-\\";
    static int frame = 0;

    ImGui::Text("%c %s", spinChars[frame % 4], label);
    frame++;
}

}  // namespace ImGuiExt
```

### Button Variants

```cpp
namespace ImGuiExt {

// Icon button with tooltip
bool IconButton(const char* icon, const char* tooltip = nullptr) {
    bool result = ImGui::Button(icon);
    if (tooltip && ImGui::IsItemHovered()) {
        ImGui::SetTooltip("%s", tooltip);
    }
    return result;
}

// Toolbar button with optional color
bool ToolBarButton(const char* icon, const char* tooltip, ImU32 color = 0) {
    if (color != 0) {
        ImGui::PushStyleColor(ImGuiCol_Text, color);
    }

    bool result = ImGui::Button(icon);

    if (color != 0) {
        ImGui::PopStyleColor();
    }

    if (tooltip && ImGui::IsItemHovered()) {
        ImGui::SetTooltip("%s", tooltip);
    }

    return result;
}

// Dimmed/subtle button
bool DimmedButton(const char* label) {
    ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0, 0, 0, 0));
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered,
        ImGui::GetStyleColorVec4(ImGuiCol_ButtonHovered));
    ImGui::PushStyleColor(ImGuiCol_ButtonActive,
        ImGui::GetStyleColorVec4(ImGuiCol_ButtonActive));

    bool result = ImGui::Button(label);

    ImGui::PopStyleColor(3);
    return result;
}

// Toggle button with active state indicator
bool DimmedButtonToggle(const char* label, bool* active) {
    if (*active) {
        ImGui::PushStyleColor(ImGuiCol_Border,
            ImGui::GetStyleColorVec4(ImGuiCol_HeaderActive));
        ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);
    }

    bool result = DimmedButton(label);
    if (result) {
        *active = !*active;
    }

    if (*active) {
        ImGui::PopStyleVar();
        ImGui::PopStyleColor();
    }

    return result;
}

// Large description button (like welcome screen buttons)
bool DescriptionButton(const char* title, const char* description,
                       const char* icon, ImVec2 size = ImVec2(0, 0)) {
    ImGui::PushID(title);

    ImVec2 textSize = ImGui::CalcTextSize(description);
    ImVec2 buttonSize = size;
    if (buttonSize.x == 0) buttonSize.x = ImGui::GetContentRegionAvail().x;
    if (buttonSize.y == 0) buttonSize.y = 60.0f;

    bool result = ImGui::Button("##descbtn", buttonSize);

    // Draw content over button
    ImVec2 min = ImGui::GetItemRectMin();
    ImVec2 max = ImGui::GetItemRectMax();
    ImDrawList* drawList = ImGui::GetWindowDrawList();

    // Icon
    ImVec2 iconPos = ImVec2(min.x + 10, min.y + (buttonSize.y - 20) / 2);
    drawList->AddText(iconPos, ImGui::GetColorU32(ImGuiCol_Text), icon);

    // Title
    ImVec2 titlePos = ImVec2(min.x + 40, min.y + 8);
    drawList->AddText(titlePos, ImGui::GetColorU32(ImGuiCol_Text), title);

    // Description
    ImVec2 descPos = ImVec2(min.x + 40, min.y + 28);
    drawList->AddText(descPos,
        ImGui::GetColorU32(ImGuiCol_TextDisabled), description);

    ImGui::PopID();
    return result;
}

}  // namespace ImGuiExt
```

### Input Variants

```cpp
namespace ImGuiExt {

// Input with prefix label
bool InputTextWithPrefix(const char* label, const char* prefix,
                         char* buf, size_t bufSize) {
    ImGui::PushID(label);

    float prefixWidth = ImGui::CalcTextSize(prefix).x + 8;

    ImGui::TextUnformatted(prefix);
    ImGui::SameLine(0, 0);

    ImGui::SetNextItemWidth(ImGui::GetContentRegionAvail().x);
    bool result = ImGui::InputText("##input", buf, bufSize);

    ImGui::PopID();
    return result;
}

// Hex input
bool InputHex(const char* label, uint64_t* value) {
    char buf[32];
    snprintf(buf, sizeof(buf), "%016llX", *value);

    if (InputTextWithPrefix(label, "0x", buf, sizeof(buf))) {
        *value = strtoull(buf, nullptr, 16);
        return true;
    }
    return false;
}

// File picker input
bool InputFilePicker(const char* label, std::string& path,
                     const char* filter = nullptr) {
    ImGui::PushID(label);

    char buf[1024];
    strncpy(buf, path.c_str(), sizeof(buf));

    float buttonWidth = 30.0f;
    ImGui::SetNextItemWidth(ImGui::GetContentRegionAvail().x - buttonWidth - 4);

    bool changed = ImGui::InputText("##path", buf, sizeof(buf));
    if (changed) path = buf;

    ImGui::SameLine();
    if (ImGui::Button("...", ImVec2(buttonWidth, 0))) {
        // Open native file dialog
        // path = openFileDialog(filter);
        changed = true;
    }

    ImGui::PopID();
    return changed;
}

}  // namespace ImGuiExt
```

### Container Widgets

```cpp
namespace ImGuiExt {

// Box container with border
void BeginBox() {
    ImGui::BeginGroup();
    ImGui::PushStyleVar(ImGuiStyleVar_CellPadding, ImVec2(8, 8));
    ImGui::BeginTable("##box", 1,
        ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg);
    ImGui::TableNextRow();
    ImGui::TableNextColumn();
}

void EndBox() {
    ImGui::EndTable();
    ImGui::PopStyleVar();
    ImGui::EndGroup();
}

// Collapsible subwindow
bool BeginSubWindow(const char* label, bool* open = nullptr) {
    ImGui::PushStyleVar(ImGuiStyleVar_ChildRounding, 4.0f);
    ImGui::PushStyleColor(ImGuiCol_ChildBg,
        ImGui::GetStyleColorVec4(ImGuiCol_FrameBg));

    bool visible = ImGui::BeginChild(label, ImVec2(0, 0), true);

    // Draw header
    if (visible) {
        ImGui::TextUnformatted(label);
        ImGui::Separator();
    }

    return visible;
}

void EndSubWindow() {
    ImGui::EndChild();
    ImGui::PopStyleColor();
    ImGui::PopStyleVar();
}

}  // namespace ImGuiExt
```

---

## 7. Typography and Icons

### Font Loading with FreeType

```cpp
void loadFonts() {
    ImGuiIO& io = ImGui::GetIO();
    float scale = getGlobalScale();
    float fontSize = 14.0f * scale;

    // Configure FreeType for better rendering
    static ImFontConfig fontConfig;
    fontConfig.FontBuilderFlags = ImGuiFreeTypeBuilderFlags_LightHinting;
    fontConfig.OversampleH = 2;
    fontConfig.OversampleV = 2;
    fontConfig.PixelSnapH = true;

    // Primary font (JetBrains Mono or similar)
    io.Fonts->AddFontFromFileTTF("fonts/JetBrainsMono-Regular.ttf",
                                  fontSize, &fontConfig);

    // Merge icon fonts
    static ImFontConfig iconConfig;
    iconConfig.MergeMode = true;
    iconConfig.PixelSnapH = true;
    iconConfig.GlyphOffset = ImVec2(0, 2);  // Adjust vertical alignment

    static const ImWchar iconRanges[] = { 0xE000, 0xF8FF, 0 };
    io.Fonts->AddFontFromFileTTF("fonts/codicons.ttf",
                                  fontSize, &iconConfig, iconRanges);

    // Build font atlas
    io.Fonts->Build();
}
```

### Icon System

```cpp
// Define icon constants (from codicons.ttf)
namespace Icons {
    constexpr const char* File           = "\xEE\x98\xB0";  // U+E630
    constexpr const char* FileBinary     = "\xEE\x98\xB1";
    constexpr const char* Folder         = "\xEE\x98\xB7";
    constexpr const char* Save           = "\xEE\xA0\x80";
    constexpr const char* Search         = "\xEE\xA0\x86";
    constexpr const char* Settings       = "\xEE\xA0\x8C";
    constexpr const char* Close          = "\xEE\xA0\x86";
    constexpr const char* Add            = "\xEE\x99\x86";
    constexpr const char* Remove         = "\xEE\x99\x87";
    constexpr const char* Edit           = "\xEE\x99\x84";
    constexpr const char* Refresh        = "\xEE\x9E\xBC";
    constexpr const char* Check          = "\xEE\x98\xAB";
    constexpr const char* Error          = "\xEE\x9A\x80";
    constexpr const char* Warning        = "\xEE\x9A\x82";
    constexpr const char* Info           = "\xEE\x9A\x85";
    constexpr const char* Pin            = "\xEE\x9C\xB7";
    constexpr const char* Unpin          = "\xEE\x9C\xB8";
    constexpr const char* Copy           = "\xEE\x9C\xB0";
    constexpr const char* Paste          = "\xEE\x9C\xB1";
    constexpr const char* Undo           = "\xEE\x9E\xA4";
    constexpr const char* Redo           = "\xEE\x9E\xA5";
}

// Usage
if (ImGuiExt::IconButton(Icons::Save, "Save file")) {
    saveCurrentFile();
}
```

### Font Variants

```cpp
class FontManager {
public:
    void push(FontType type) {
        ImGui::PushFont(m_fonts[static_cast<int>(type)]);
    }

    void pop() {
        ImGui::PopFont();
    }

    enum class FontType {
        Regular,
        Bold,
        Italic,
        Monospace,
        Large,
        Small
    };

private:
    ImFont* m_fonts[6];
};

// Usage
FontManager::push(FontManager::FontType::Bold);
ImGui::TextUnformatted("Important text");
FontManager::pop();
```

---

## 8. Interaction Patterns

### Shortcut System

```cpp
struct Key {
    ImGuiKey key;
    ImGuiModFlags mods;

    Key operator+(Key other) const {
        return {other.key, mods | other.mods};
    }

    bool matches(ImGuiIO& io) const {
        if (!ImGui::IsKeyPressed(key)) return false;

        bool ctrl  = (mods & ImGuiModFlags_Ctrl)  ? io.KeyCtrl  : !io.KeyCtrl;
        bool shift = (mods & ImGuiModFlags_Shift) ? io.KeyShift : !io.KeyShift;
        bool alt   = (mods & ImGuiModFlags_Alt)   ? io.KeyAlt   : !io.KeyAlt;
        bool super = (mods & ImGuiModFlags_Super) ? io.KeySuper : !io.KeySuper;

        return ctrl && shift && alt && super;
    }
};

// Modifier keys
constexpr Key CTRL  = {ImGuiKey_None, ImGuiModFlags_Ctrl};
constexpr Key SHIFT = {ImGuiKey_None, ImGuiModFlags_Shift};
constexpr Key ALT   = {ImGuiKey_None, ImGuiModFlags_Alt};

// Platform-aware command key (Cmd on macOS, Ctrl elsewhere)
#ifdef __APPLE__
constexpr Key CMD = {ImGuiKey_None, ImGuiModFlags_Super};
#else
constexpr Key CMD = CTRL;
#endif

namespace Keys {
    constexpr Key A = {ImGuiKey_A, 0};
    constexpr Key S = {ImGuiKey_S, 0};
    constexpr Key Z = {ImGuiKey_Z, 0};
    constexpr Key Y = {ImGuiKey_Y, 0};
    // ... more keys
}

class ShortcutManager {
public:
    using Callback = std::function<void()>;

    void registerShortcut(Key shortcut, Callback callback,
                          const std::string& description) {
        m_shortcuts.push_back({shortcut, callback, description});
    }

    void process() {
        ImGuiIO& io = ImGui::GetIO();

        // Skip if typing in a text field
        if (io.WantTextInput) return;

        for (auto& s : m_shortcuts) {
            if (s.shortcut.matches(io)) {
                s.callback();
                break;
            }
        }
    }

private:
    struct Shortcut {
        Key shortcut;
        Callback callback;
        std::string description;
    };
    std::vector<Shortcut> m_shortcuts;
};

// Usage
ShortcutManager shortcuts;
shortcuts.registerShortcut(CMD + Keys::S, []{ saveFile(); }, "Save");
shortcuts.registerShortcut(CMD + Keys::Z, []{ undo(); }, "Undo");
shortcuts.registerShortcut(CMD + SHIFT + Keys::Z, []{ redo(); }, "Redo");
```

### Context Menu Pattern

```cpp
void drawWithContextMenu() {
    ImGui::TextUnformatted("Right-click me");

    if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
        ImGui::OpenPopup("ItemContextMenu");
    }

    if (ImGui::BeginPopup("ItemContextMenu")) {
        if (ImGui::MenuItem("Copy", "Ctrl+C")) {
            copyToClipboard();
        }
        if (ImGui::MenuItem("Paste", "Ctrl+V")) {
            pasteFromClipboard();
        }
        ImGui::Separator();
        if (ImGui::MenuItem("Delete", "Del")) {
            deleteItem();
        }
        ImGui::EndPopup();
    }
}
```

### Drag and Drop

```cpp
// Source
if (ImGui::BeginDragDropSource()) {
    MyData* data = getSelectedData();
    ImGui::SetDragDropPayload("MY_DATA_TYPE", &data, sizeof(MyData*));
    ImGui::TextUnformatted("Dragging item...");
    ImGui::EndDragDropSource();
}

// Target
if (ImGui::BeginDragDropTarget()) {
    if (const ImGuiPayload* payload =
            ImGui::AcceptDragDropPayload("MY_DATA_TYPE")) {
        MyData* data = *(MyData**)payload->Data;
        handleDrop(data);
    }
    ImGui::EndDragDropTarget();
}
```

### Selection System

```cpp
class Selection {
public:
    void set(uint64_t start, uint64_t end) {
        m_start = std::min(start, end);
        m_end = std::max(start, end);
        m_valid = true;
        m_changed = true;
    }

    void clear() {
        m_valid = false;
        m_changed = true;
    }

    bool isValid() const { return m_valid; }
    bool hasChanged() {
        bool changed = m_changed;
        m_changed = false;
        return changed;
    }

    uint64_t start() const { return m_start; }
    uint64_t end() const { return m_end; }
    uint64_t size() const { return m_end - m_start + 1; }

    bool contains(uint64_t addr) const {
        return m_valid && addr >= m_start && addr <= m_end;
    }

private:
    uint64_t m_start = 0;
    uint64_t m_end = 0;
    bool m_valid = false;
    bool m_changed = false;
};
```

### Undo/Redo System

```cpp
class Operation {
public:
    virtual ~Operation() = default;
    virtual void undo() = 0;
    virtual void redo() = 0;
    virtual std::string description() const = 0;
};

class UndoStack {
public:
    void push(std::unique_ptr<Operation> op) {
        // Clear redo stack
        while (m_position < m_operations.size()) {
            m_operations.pop_back();
        }

        m_operations.push_back(std::move(op));
        m_position++;
    }

    bool canUndo() const { return m_position > 0; }
    bool canRedo() const { return m_position < m_operations.size(); }

    void undo() {
        if (!canUndo()) return;
        m_position--;
        m_operations[m_position]->undo();
    }

    void redo() {
        if (!canRedo()) return;
        m_operations[m_position]->redo();
        m_position++;
    }

    // Group multiple operations into one undo step
    void beginGroup(const std::string& name) {
        m_groupName = name;
        m_groupStart = m_position;
    }

    void endGroup() {
        if (m_position > m_groupStart) {
            // Create compound operation from grouped ops
            // ... implementation
        }
    }

private:
    std::vector<std::unique_ptr<Operation>> m_operations;
    size_t m_position = 0;
    std::string m_groupName;
    size_t m_groupStart = 0;
};
```

---

## 9. Event System

### Event Definition

```cpp
// Event base with compile-time type checking
template<typename... Args>
class Event {
public:
    using Callback = std::function<void(Args...)>;
    using Token = size_t;

    static Token subscribe(Callback callback) {
        Token token = s_nextToken++;
        s_callbacks[token] = callback;
        return token;
    }

    static void unsubscribe(Token token) {
        s_callbacks.erase(token);
    }

    static void post(Args... args) {
        for (auto& [token, callback] : s_callbacks) {
            callback(args...);
        }
    }

private:
    static inline std::map<Token, Callback> s_callbacks;
    static inline Token s_nextToken = 0;
};

// Define specific events
using EventDataChanged = Event<>;
using EventSelectionChanged = Event<uint64_t, uint64_t>;
using EventThemeChanged = Event<>;
using EventFileOpened = Event<const std::string&>;
using EventFileClosed = Event<>;
using EventWindowFocused = Event<bool>;
```

### Event Usage

```cpp
// Subscribe
auto token = EventDataChanged::subscribe([]() {
    refreshView();
});

// Later, unsubscribe
EventDataChanged::unsubscribe(token);

// Post event
EventDataChanged::post();
EventSelectionChanged::post(startAddr, endAddr);
```

### Request Pattern

```cpp
// For actions that should be handled by a specific component
template<typename... Args>
class Request {
public:
    using Handler = std::function<void(Args...)>;

    static void setHandler(Handler handler) {
        s_handler = handler;
    }

    static void post(Args... args) {
        if (s_handler) {
            s_handler(args...);
        }
    }

private:
    static inline Handler s_handler;
};

using RequestOpenFile = Request<const std::string&>;
using RequestJumpToAddress = Request<uint64_t>;
using RequestOpenPopup = Request<const std::string&>;
```

---

## 10. Best Practices

### Performance

1. **Minimize draw calls**: Batch similar widgets together
2. **Use clipper for long lists**: `ImGuiListClipper` for virtualized lists
3. **Lazy loading**: Only compute visible content
4. **Cache expensive calculations**: Store results between frames
5. **Adaptive frame rate**: Lower FPS when idle

```cpp
// Virtualized list
void drawLargeList(const std::vector<Item>& items) {
    ImGuiListClipper clipper;
    clipper.Begin(items.size());

    while (clipper.Step()) {
        for (int i = clipper.DisplayStart; i < clipper.DisplayEnd; i++) {
            drawItem(items[i]);
        }
    }
}
```

### Code Organization

```
src/
├── core/
│   ├── window.cpp          # Window management
│   ├── theme.cpp           # Theming system
│   └── events.cpp          # Event system
├── ui/
│   ├── widgets/            # Custom widgets
│   │   ├── buttons.cpp
│   │   ├── inputs.cpp
│   │   └── containers.cpp
│   ├── views/              # View panels
│   │   ├── view.cpp        # Base view class
│   │   ├── main_view.cpp
│   │   └── settings_view.cpp
│   └── popups/             # Modal dialogs
│       ├── popup.cpp       # Base popup class
│       └── file_picker.cpp
├── api/
│   ├── shortcuts.cpp       # Shortcut management
│   ├── layout.cpp          # Layout persistence
│   └── settings.cpp        # Settings management
└── main.cpp
```

### Accessibility

1. **Keyboard navigation**: Ensure all actions are keyboard-accessible
2. **High contrast**: Support high-contrast themes
3. **Scalable UI**: Respect system DPI settings
4. **Tooltips**: Provide tooltips for all interactive elements
5. **Focus indicators**: Clear visual focus states

### Error Handling

```cpp
// Toast notifications for non-blocking errors
class Toast {
public:
    static void show(const std::string& message, ToastType type) {
        s_toasts.push_back({message, type, 4.0f});  // 4 second display
    }

    static void draw() {
        float y = 10.0f;
        auto it = s_toasts.begin();
        while (it != s_toasts.end()) {
            it->remaining -= ImGui::GetIO().DeltaTime;
            if (it->remaining <= 0) {
                it = s_toasts.erase(it);
                continue;
            }

            drawToast(*it, y);
            y += 50.0f;
            ++it;
        }
    }

private:
    struct ToastData {
        std::string message;
        ToastType type;
        float remaining;
    };
    static inline std::vector<ToastData> s_toasts;
};
```

---

## Quick Reference: Color Palette

### Dark Theme Base Colors

| Element | Hex | RGB |
|---------|-----|-----|
| Background | #1E1E1E | 30, 30, 30 |
| Surface | #252526 | 37, 37, 38 |
| Border | #3C3C3C | 60, 60, 60 |
| Primary | #007ACC | 0, 122, 204 |
| Accent | #1C97EA | 28, 151, 234 |
| Text | #D4D4D4 | 212, 212, 212 |
| Text Dim | #808080 | 128, 128, 128 |
| Success | #89D185 | 137, 209, 133 |
| Warning | #CCA700 | 204, 167, 0 |
| Error | #F14C4C | 241, 76, 76 |

### Style Dimensions

| Property | Value |
|----------|-------|
| Window Rounding | 4px |
| Frame Rounding | 2px |
| Window Padding | 8px |
| Frame Padding | 6px, 4px |
| Item Spacing | 8px, 4px |
| Scrollbar Size | 14px |
| Font Size | 14px |

---

## Conclusion

This guide captures the essential patterns and practices used in ImHex's UI. The key principles are:

1. **Layered architecture** separating platform, widgets, views, and application logic
2. **Consistent theming** with a well-defined color palette and style variables
3. **DPI-aware design** that scales properly on all displays
4. **Event-driven communication** between components
5. **Immediate-mode philosophy** embracing ImGui's paradigm fully

Following these patterns will produce a professional, responsive, and visually polished application that matches the quality of ImHex.
