#pragma once

#include "core/config.h"

#include <functional>

namespace kuf {

// Settings dialog.
class SettingsDialog {
      public:
	SettingsDialog();

	// Opens the dialog.
	void open();

	// Renders the dialog. Returns true while open.
	bool draw();

	// Settings access.
	const AppConfig &config() const { return config_; }
	AppConfig &config() { return config_; }

	// Applies the current settings (e.g., theme).
	void apply();

	// Load config from disk.
	void load();

	// Save config to disk.
	void save();

	void setOnFontSizeChanged(std::function<void(float)> callback) {
		onFontSizeChanged_ = std::move(callback);
	}

	void setOnGamePathsChanged(std::function<void()> callback) {
		onGamePathsChanged_ = std::move(callback);
	}

      private:
	void applyTheme();
	void applyDarkTheme();
	void applyLightTheme();
	void applyClassicTheme();

	AppConfig config_;
	AppConfig pendingConfig_;
	bool open_ = false;
	float appliedFontSize_ = 17.0f;
	char crusadersPathBuf_[512] = {};
	char heroesPathBuf_[512] = {};
	std::function<void(float)> onFontSizeChanged_;
	std::function<void()> onGamePathsChanged_;
};

} // namespace kuf
