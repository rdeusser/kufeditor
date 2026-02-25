#pragma once

#include <string>
#include <vector>

namespace kuf {

// Application theme.
enum class Theme { Dark, Light, Classic };

// Active game selection.
enum class ActiveGame { None, Crusaders, Heroes };

// Application configuration.
struct AppConfig {
	Theme theme = Theme::Dark;
	float fontSize = 17.0f;
	int maxRecentFiles = 10;
	std::vector<std::string> recentFiles;
	ActiveGame activeGame = ActiveGame::None;
	std::string crusadersPath;
	std::string heroesPath;
};

// Returns the platform-specific config directory, creating it if needed.
std::string getConfigDir();

// Returns the full path to the config file.
std::string getConfigPath();

// Loads configuration from disk.
AppConfig loadConfig();

// Saves configuration to disk.
void saveConfig(const AppConfig &config);

} // namespace kuf
