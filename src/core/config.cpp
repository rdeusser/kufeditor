#include "core/config.h"

#include <libconfig.h++>

#include <cstdlib>
#include <filesystem>

namespace kuf {

std::string getConfigDir() {
	std::string dir;

#if defined(__APPLE__)
	if (const char *home = std::getenv("HOME")) {
		dir = std::string(home) +
		      "/Library/Application Support/kufeditor";
	}
#elif defined(_WIN32)
	if (const char *appdata = std::getenv("APPDATA")) {
		dir = std::string(appdata) + "/kufeditor";
	}
#else
	if (const char *xdg = std::getenv("XDG_CONFIG_HOME")) {
		dir = std::string(xdg) + "/kufeditor";
	} else if (const char *home = std::getenv("HOME")) {
		dir = std::string(home) + "/.config/kufeditor";
	}
#endif

	if (dir.empty()) {
		dir = ".";
	}

	std::filesystem::create_directories(dir);
	return dir;
}

std::string getConfigPath() { return getConfigDir() + "/config.cfg"; }

AppConfig loadConfig() {
	AppConfig config;
	libconfig::Config cfg;

	try {
		cfg.readFile(getConfigPath().c_str());

		int theme = 0;
		if (cfg.lookupValue("theme", theme) && theme >= 0 &&
		    theme <= 2) {
			config.theme = static_cast<Theme>(theme);
		}

		cfg.lookupValue("fontSize", config.fontSize);
		cfg.lookupValue("maxRecentFiles", config.maxRecentFiles);

		int activeGame = 0;
		if (cfg.lookupValue("activeGame", activeGame) &&
		    activeGame >= 0 && activeGame <= 2) {
			config.activeGame = static_cast<ActiveGame>(activeGame);
		}

		std::string crusadersPath, heroesPath;
		if (cfg.lookupValue("crusadersPath", crusadersPath)) {
			config.crusadersPath = crusadersPath;
		}
		if (cfg.lookupValue("heroesPath", heroesPath)) {
			config.heroesPath = heroesPath;
		}

		if (cfg.exists("recentFiles")) {
			const libconfig::Setting &files =
			    cfg.lookup("recentFiles");
			for (int i = 0; i < files.getLength(); ++i) {
				config.recentFiles.push_back(files[i].c_str());
			}
		}
	} catch (const libconfig::FileIOException &) {
		// Config file doesn't exist yet.
	} catch (const libconfig::ParseException &) {
		// Malformed config file.
	}

	return config;
}

void saveConfig(const AppConfig &config) {
	libconfig::Config cfg;
	libconfig::Setting &root = cfg.getRoot();

	root.add("theme", libconfig::Setting::TypeInt) =
	    static_cast<int>(config.theme);
	root.add("fontSize", libconfig::Setting::TypeFloat) =
	    static_cast<double>(config.fontSize);
	root.add("maxRecentFiles", libconfig::Setting::TypeInt) =
	    config.maxRecentFiles;
	root.add("activeGame", libconfig::Setting::TypeInt) =
	    static_cast<int>(config.activeGame);
	root.add("crusadersPath", libconfig::Setting::TypeString) =
	    config.crusadersPath;
	root.add("heroesPath", libconfig::Setting::TypeString) =
	    config.heroesPath;

	libconfig::Setting &files =
	    root.add("recentFiles", libconfig::Setting::TypeArray);
	for (const auto &path : config.recentFiles) {
		files.add(libconfig::Setting::TypeString) = path;
	}

	try {
		cfg.writeFile(getConfigPath().c_str());
	} catch (const libconfig::FileIOException &) {
		// Failed to write config.
	}
}

} // namespace kuf
