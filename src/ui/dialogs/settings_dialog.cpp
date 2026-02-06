#include "ui/dialogs/settings_dialog.h"

#include <imgui.h>

namespace kuf {

SettingsDialog::SettingsDialog() {
    pendingConfig_ = config_;
}

void SettingsDialog::open() {
    open_ = true;
    pendingConfig_ = config_;
}

bool SettingsDialog::draw() {
    if (!open_) return false;

    ImGui::SetNextWindowSize(ImVec2(400, 300), ImGuiCond_FirstUseEver);

    if (!ImGui::Begin("Settings", &open_)) {
        ImGui::End();
        return open_;
    }

    if (ImGui::BeginTabBar("SettingsTabs")) {
        // Appearance tab.
        if (ImGui::BeginTabItem("Appearance")) {
            ImGui::Text("Theme");
            const char* themes[] = {"Dark", "Light", "Classic"};
            int currentTheme = static_cast<int>(pendingConfig_.theme);
            if (ImGui::Combo("##Theme", &currentTheme, themes, 3)) {
                pendingConfig_.theme = static_cast<Theme>(currentTheme);
            }

            ImGui::Spacing();
            ImGui::SliderFloat("Font Size", &pendingConfig_.fontSize, 10.0f, 24.0f, "%.0f");

            ImGui::EndTabItem();
        }

        // General tab.
        if (ImGui::BeginTabItem("General")) {
            ImGui::SliderInt("Max Recent Files", &pendingConfig_.maxRecentFiles, 5, 20);

            ImGui::EndTabItem();
        }

        ImGui::EndTabBar();
    }

    ImGui::Separator();

    if (ImGui::Button("Apply", ImVec2(80, 0))) {
        config_ = pendingConfig_;
        apply();
        save();
    }
    ImGui::SameLine();
    if (ImGui::Button("OK", ImVec2(80, 0))) {
        config_ = pendingConfig_;
        apply();
        save();
        open_ = false;
    }
    ImGui::SameLine();
    if (ImGui::Button("Cancel", ImVec2(80, 0))) {
        pendingConfig_ = config_;
        open_ = false;
    }

    ImGui::End();
    return open_;
}

void SettingsDialog::apply() {
    applyTheme();
}

void SettingsDialog::applyTheme() {
    switch (config_.theme) {
    case Theme::Dark:
        applyDarkTheme();
        break;
    case Theme::Light:
        applyLightTheme();
        break;
    case Theme::Classic:
        applyClassicTheme();
        break;
    }
}

void SettingsDialog::applyDarkTheme() {
    ImGui::StyleColorsDark();
}

void SettingsDialog::applyLightTheme() {
    ImGui::StyleColorsLight();
}

void SettingsDialog::applyClassicTheme() {
    ImGui::StyleColorsClassic();
}

void SettingsDialog::load() {
    config_ = loadConfig();
    pendingConfig_ = config_;
}

void SettingsDialog::save() {
    saveConfig(config_);
}

} // namespace kuf
