#pragma once

#include "core/config.h"

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
    const AppConfig& config() const { return config_; }
    AppConfig& config() { return config_; }

    // Applies the current settings (e.g., theme).
    void apply();

    // Load config from disk.
    void load();

    // Save config to disk.
    void save();

private:
    void applyTheme();
    void applyDarkTheme();
    void applyLightTheme();
    void applyClassicTheme();

    AppConfig config_;
    AppConfig pendingConfig_;
    bool open_ = false;
};

} // namespace kuf
