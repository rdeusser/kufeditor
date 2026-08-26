cmake_minimum_required(VERSION 3.20)

# Enable CMAKE_MSVC_DEBUG_INFORMATION_FORMAT support (CMake 3.25+).
if(POLICY CMP0141)
    cmake_policy(SET CMP0141 NEW)
    set(CMAKE_POLICY_DEFAULT_CMP0141 NEW)
endif()

project(kufeditor VERSION 1.0.0 LANGUAGES CXX)

if(APPLE)
    enable_language(OBJCXX)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# On macOS, ensure Apple's native ar/ranlib are used to avoid GNU archive
# format incompatibility when Homebrew binutils or ccache are in the PATH.
if(APPLE AND EXISTS /usr/bin/ar)
    set(CMAKE_AR /usr/bin/ar)
    set(CMAKE_RANLIB /usr/bin/ranlib)
endif()

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
    # Keep the legacy coexistence build compatible with imgui-node-editor.
    GIT_TAG aeaaee8090a7f9751ee9684de979a0ea673c935f
)
FetchContent_MakeAvailable(imgui)

# libconfig: try pkg-config first (macOS/Linux), fall back to find_package (vcpkg/Windows).
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(LIBCONFIG IMPORTED_TARGET libconfig++)
endif()

if(NOT LIBCONFIG_FOUND)
    find_package(libconfig CONFIG REQUIRED)
    set(LIBCONFIG_LINK_TARGET libconfig::libconfig++)
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

# miniz for zip archive support
# Disable sccache for miniz on MSVC. Despite CMAKE_MSVC_DEBUG_INFORMATION_FORMAT
# being set to Embedded, miniz's C files end up with both /Zi and /Z7 flags.
# /Z7 wins (last-wins) so no PDB is generated, but sccache still expects the
# PDB referenced by /Fd and errors when it doesn't exist.
if(MSVC)
    set(_saved_c_launcher "${CMAKE_C_COMPILER_LAUNCHER}")
    set(CMAKE_C_COMPILER_LAUNCHER "")
endif()
FetchContent_Declare(
    miniz
    GIT_REPOSITORY https://github.com/richgel999/miniz.git
    GIT_TAG 3.1.1
)
FetchContent_MakeAvailable(miniz)
if(MSVC)
    set(CMAKE_C_COMPILER_LAUNCHER "${_saved_c_launcher}")
endif()

# Cleave-generated parsers
file(GLOB CLEAVE_PARSER_SOURCES src/parsers/*.cpp)

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

# imgui-node-editor
FetchContent_Declare(
    imgui_node_editor
    GIT_REPOSITORY https://github.com/thedmd/imgui-node-editor.git
    GIT_TAG b302971455b3719ec9b5fb94b2f92d27c62b9ff0
    PATCH_COMMAND ${CMAKE_COMMAND} -P ${CMAKE_SOURCE_DIR}/cmake/patch_node_editor.cmake
)
FetchContent_MakeAvailable(imgui_node_editor)

add_library(imgui_node_editor_lib STATIC
    ${imgui_node_editor_SOURCE_DIR}/imgui_node_editor.cpp
    ${imgui_node_editor_SOURCE_DIR}/imgui_node_editor_api.cpp
    ${imgui_node_editor_SOURCE_DIR}/imgui_canvas.cpp
    ${imgui_node_editor_SOURCE_DIR}/crude_json.cpp
)
target_include_directories(imgui_node_editor_lib PUBLIC ${imgui_node_editor_SOURCE_DIR})
target_link_libraries(imgui_node_editor_lib PUBLIC imgui)

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

# iconv for CP949/Korean text encoding conversion.
find_package(Iconv REQUIRED)

# Main executable
add_executable(kufeditor
    src/main.cpp
    src/core/window.cpp
    src/core/imgui_context.cpp
    src/core/application.cpp
    src/core/config.cpp
    src/core/recent_files.cpp
    src/core/tab_manager.cpp
    src/core/text_encoding.cpp
    src/core/name_dictionary.cpp
    src/core/steam.cpp
    src/formats/sox_binary.cpp
    src/formats/sox_skill_info.cpp
    src/formats/sox_text.cpp
    src/formats/save_format.cpp
    src/formats/sox_encoding.cpp
    src/formats/stg_format.cpp
    src/formats/stg_script_catalog.cpp
    src/ui/views/home_view.cpp
    src/ui/views/validation_log.cpp
    src/ui/tabs/skill_editor_tab.cpp
    src/ui/tabs/troop_editor_tab.cpp
    src/ui/tabs/text_editor_tab.cpp
    src/ui/tabs/save_editor_tab.cpp
    src/ui/tabs/stg_editor_tab.cpp
    src/ui/node_graph/stg_node_graph.cpp
    src/ui/dialogs/settings_dialog.cpp
    src/undo/undo_stack.cpp
    src/core/json.cpp
    src/core/zip_archive.cpp
    src/mods/backup_manager.cpp
    src/mods/mod_manager.cpp
    src/ui/views/mod_manager_view.cpp
    src/ui/views/patch_editor_view.cpp
    src/patches/exe_patch.cpp
    ${CLEAVE_PARSER_SOURCES}
    ${PLATFORM_SOURCES}
)
target_link_libraries(kufeditor PRIVATE
    imgui
    imgui_node_editor_lib
    glfw
    Iconv::Iconv
    ${LIBCONFIG_LINK_TARGET}
    miniz
)
target_include_directories(kufeditor PRIVATE src)

# On Windows, build as a GUI application (no console window).
if(WIN32)
    set_target_properties(kufeditor PROPERTIES WIN32_EXECUTABLE TRUE)
endif()

# Silence OpenGL deprecation warnings on macOS.
if(APPLE)
    target_compile_definitions(kufeditor PRIVATE GL_SILENCE_DEPRECATION)
endif()

# Platform-specific libraries for file dialogs
if(WIN32)
    target_link_libraries(kufeditor PRIVATE comdlg32)
elseif(APPLE)
    target_link_libraries(kufeditor PRIVATE "-framework Cocoa" "-framework UniformTypeIdentifiers")
endif()

# Tests
enable_testing()
add_executable(kufeditor_tests
    test/main_test.cpp
    test/save_format_test.cpp
    test/sox_binary_test.cpp
    test/sox_encoding_test.cpp
    test/sox_skill_info_test.cpp
    test/stg_format_test.cpp
    src/core/text_encoding.cpp
    src/formats/save_format.cpp
    src/formats/sox_binary.cpp
    src/formats/sox_skill_info.cpp
    src/formats/sox_encoding.cpp
    src/formats/stg_format.cpp
    src/formats/stg_script_catalog.cpp
    ${CLEAVE_PARSER_SOURCES}
)
target_link_libraries(kufeditor_tests PRIVATE
    Catch2::Catch2WithMain
    Iconv::Iconv
    imgui
)
target_include_directories(kufeditor_tests PRIVATE src)
include(CTest)
include(Catch)
catch_discover_tests(kufeditor_tests)
