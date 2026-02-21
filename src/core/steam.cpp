#include "core/steam.h"

#include <filesystem>

namespace kuf {

#ifdef _WIN32

namespace fs = std::filesystem;

namespace {

const char *kSteamPatterns[] = {
    "Steam\\steamapps\\common",
    "Program Files\\Steam\\steamapps\\common",
    "Program Files (x86)\\Steam\\steamapps\\common",
};

struct GameDef {
	const char *displayName;
	const char *folder;
};

const GameDef kGames[] = {
    {"Kingdom Under Fire The Crusaders", "KUF Crusader"},
    {"Kingdom Under Fire Heroes", "KUF Heroes"},
};

} // namespace

std::vector<SteamGame> detectSteamGames() {
	std::vector<SteamGame> results;

	for (char drive = 'A'; drive <= 'Z'; ++drive) {
		std::string driveRoot = std::string(1, drive) + ":\\";
		if (!fs::is_directory(driveRoot)) continue;

		for (const char *pattern : kSteamPatterns) {
			fs::path steamCommon = fs::path(driveRoot) / pattern;
			if (!fs::is_directory(steamCommon)) continue;

			for (const auto &game : kGames) {
				fs::path gameDir = steamCommon / game.folder;
				if (!fs::is_directory(gameDir / "Data" / "SOX"))
					continue;

				std::string pathStr = gameDir.string();
				bool duplicate = false;
				for (const auto &existing : results)
					if (existing.path == pathStr) {
						duplicate = true;
						break;
					}
				if (!duplicate)
					results.push_back(
					    {game.displayName, pathStr});
			}
		}
	}

	return results;
}

std::string findSteamSoxDirectory() {
	for (char drive = 'A'; drive <= 'Z'; ++drive) {
		std::string driveRoot = std::string(1, drive) + ":\\";
		if (!fs::is_directory(driveRoot)) continue;

		for (const char *pattern : kSteamPatterns) {
			fs::path gameDir =
			    fs::path(driveRoot) / pattern / "KUF Crusader";
			if (!fs::exists(gameDir / "Kuf2Main.exe")) continue;

			fs::path soxDir = gameDir / "Data" / "SOX";
			if (fs::is_directory(soxDir)) return soxDir.string();
		}
	}

	return {};
}

#else

std::vector<SteamGame> detectSteamGames() { return {}; }
std::string findSteamSoxDirectory() { return {}; }

#endif

} // namespace kuf
