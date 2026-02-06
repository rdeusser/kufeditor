cmake_minimum_required(VERSION 3.20)
project(kufeditor VERSION 1.0.0 LANGUAGES CXX)

if(APPLE)
    enable_language(OBJCXX)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Static MSVC runtime for portable Windows builds.
if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

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

# libconfig: try pkg-config first (macOS/Linux), fall back to find_package (vcpkg/Windows).
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(LIBCONFIG IMPORTED_TARGET libconfig++)
endif()

if(NOT LIBCONFIG_FOUND)
    find_package(libconfig CONFIG REQUIRED)
    set(LIBCONFIG_LINK_TARGET libconfig::config++)
else()
    set(LIBCONFIG_LINK_TARGET PkgConfig::LIBCONFIG)
endif()

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

# Platform-specific OpenGL linking
if(WIN32)
    target_link_libraries(imgui PUBLIC opengl32)
elseif(APPLE)
    find_package(OpenGL REQUIRED)
    target_link_libraries(imgui PUBLIC OpenGL::GL)
else()
    find_package(OpenGL REQUIRED)
    target_link_libraries(imgui PUBLIC OpenGL::GL)
endif()

# Platform-specific source files.
set(PLATFORM_SOURCES src/ui/dialogs/file_dialog.cpp)
if(APPLE)
    list(APPEND PLATFORM_SOURCES src/ui/dialogs/file_dialog_macos.mm)
endif()

# Main executable
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
    src/core/config.cpp
    src/core/recent_files.cpp
    src/core/tab_manager.cpp
    src/formats/sox_binary.cpp
    src/formats/sox_text.cpp
    src/formats/sox_encoding.cpp
    src/ui/views/home_view.cpp
    src/ui/views/validation_log.cpp
    src/ui/tabs/troop_editor_tab.cpp
    src/ui/tabs/text_editor_tab.cpp
    src/ui/dialogs/settings_dialog.cpp
    src/undo/undo_stack.cpp
    ${PLATFORM_SOURCES}
)
target_link_libraries(kufeditor PRIVATE
    imgui
    glfw
    ${LIBCONFIG_LINK_TARGET}
)
target_include_directories(kufeditor PRIVATE src)

# Silence OpenGL deprecation warnings on macOS.
if(APPLE)
    target_compile_definitions(kufeditor PRIVATE GL_SILENCE_DEPRECATION)
endif()

# Platform-specific libraries for file dialogs
if(WIN32)
    target_link_libraries(kufeditor PRIVATE comdlg32)
elseif(APPLE)
    target_link_libraries(kufeditor PRIVATE "-framework Cocoa")
endif()

# Tests
enable_testing()
add_executable(kufeditor_tests
    test/main_test.cpp
    test/sox_binary_test.cpp
    test/sox_encoding_test.cpp
    src/formats/sox_binary.cpp
    src/formats/sox_encoding.cpp
)
target_link_libraries(kufeditor_tests PRIVATE Catch2::Catch2WithMain)
target_include_directories(kufeditor_tests PRIVATE src)
include(CTest)
include(Catch)
catch_discover_tests(kufeditor_tests)
