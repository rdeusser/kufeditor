#pragma once

#include "ui/views/view.h"

#include <functional>
#include <string>
#include <vector>

namespace kuf {

/// Home view showing game selection UI.
class HomeView : public View {
public:
    HomeView();

    void drawContent() override;

    void setOnSelectGameDirectory(std::function<void(const std::string&)> cb) {
        onSelectGameDirectory_ = std::move(cb);
    }

private:
    struct GameInfo {
        std::string name;
        std::string path;
        bool exists = false;
    };

    void detectGames();
    void drawGameButton(const GameInfo& game);

    std::vector<GameInfo> detectedGames_;
    std::function<void(const std::string&)> onSelectGameDirectory_;
    bool gamesDetected_ = false;
};

} // namespace kuf
