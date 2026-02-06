#include "ui/views/home_view.h"
#include "ui/dialogs/file_dialog.h"

#include <imgui.h>
#include <filesystem>

namespace kuf {

HomeView::HomeView() : View("Home") {}

void HomeView::drawContent() {
    if (!gamesDetected_) {
        detectGames();
        gamesDetected_ = true;
    }

    ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(12, 8));

    ImGui::Spacing();
    ImGui::TextWrapped("Welcome to KUF Editor. Select a game directory below to set it as "
                       "the default location for File > Open. Then use File > Open (Ctrl+O) "
                       "to open individual files.");
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

    // Browse button.
    if (ImGui::Button("Browse...", ImVec2(120, 0))) {
        if (auto path = FileDialog::openFolder()) {
            if (onSelectGameDirectory_) {
                onSelectGameDirectory_(*path);
            }
        }
    }
    ImGui::SameLine();
    ImGui::TextDisabled("Select a game's SOX folder");

    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

#ifdef _WIN32
    ImGui::Text("Detected Games:");
    ImGui::Spacing();

    bool anyFound = false;
    for (const auto& game : detectedGames_) {
        if (game.exists) {
            anyFound = true;
            drawGameButton(game);
        }
    }

    if (!anyFound) {
        ImGui::TextDisabled("No games found in standard Steam locations.");
        ImGui::TextDisabled("Use Browse to select a game folder manually.");
    }
#else
    ImGui::TextDisabled("Auto-detection is only available on Windows.");
    ImGui::TextDisabled("Use Browse to select a game's SOX folder.");
#endif

    ImGui::PopStyleVar();
}

void HomeView::detectGames() {
    detectedGames_.clear();

#ifdef _WIN32
    namespace fs = std::filesystem;

    // Steam paths to check.
    std::vector<std::string> steamPaths = {
        "C:\\Program Files\\Steam\\steamapps\\common",
        "C:\\Program Files (x86)\\Steam\\steamapps\\common",
        "C:\\Steam\\steamapps\\common",
        "D:\\Steam\\steamapps\\common",
        "D:\\SteamLibrary\\steamapps\\common",
        "E:\\SteamLibrary\\steamapps\\common"
    };

    // Games and their SOX folder names.
    std::vector<std::pair<std::string, std::string>> games = {
        {"Kingdom Under Fire The Crusaders", "Kingdom Under Fire The Crusaders"},
        {"Kingdom Under Fire Heroes", "Kingdom Under Fire Heroes"}
    };

    for (const auto& steamPath : steamPaths) {
        for (const auto& [gameName, gameFolder] : games) {
            std::string gamePath = steamPath + "\\" + gameFolder;
            std::string soxPath = gamePath + "\\SOX";

            if (fs::exists(soxPath) && fs::is_directory(soxPath)) {
                GameInfo info;
                info.name = gameName;
                info.path = soxPath;
                info.exists = true;

                // Avoid duplicates.
                bool duplicate = false;
                for (const auto& existing : detectedGames_) {
                    if (existing.path == info.path) {
                        duplicate = true;
                        break;
                    }
                }
                if (!duplicate) {
                    detectedGames_.push_back(info);
                }
            }
        }
    }
#endif
}

void HomeView::drawGameButton(const GameInfo& game) {
    ImGui::PushID(game.path.c_str());

    if (ImGui::Button("Select", ImVec2(80, 0))) {
        if (onSelectGameDirectory_) {
            onSelectGameDirectory_(game.path);
        }
    }
    ImGui::SameLine();
    ImGui::Text("%s", game.name.c_str());
    ImGui::SameLine();
    ImGui::TextDisabled("(%s)", game.path.c_str());

    ImGui::PopID();
}

} // namespace kuf
