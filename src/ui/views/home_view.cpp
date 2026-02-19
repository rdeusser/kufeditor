#include "ui/views/home_view.h"

#include <imgui.h>

#include "core/steam.h"

namespace kuf {

HomeView::HomeView() : View("Home") {}

void HomeView::drawContent() {
    if (!gamesDetected_) {
        detectGames();
        gamesDetected_ = true;
    }

    ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(12, 8));

    ImGui::Spacing();
    ImGui::TextWrapped("Welcome to KUF Editor. Use File > Open (Ctrl+O) to open game files.");
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
        ImGui::TextDisabled("Use File > Set Game Directory to select a game folder manually.");
    }
#else
    ImGui::TextDisabled("Auto-detection is only available on Windows.");
    ImGui::TextDisabled("Use File > Open to open game files directly.");
#endif

    ImGui::PopStyleVar();
}

void HomeView::detectGames() {
    detectedGames_.clear();

    for (const auto& game : detectSteamGames())
        detectedGames_.push_back({game.name, game.path, true});
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
