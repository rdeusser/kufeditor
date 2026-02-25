#include "ui/dialogs/settings_dialog.h"

#include "core/steam.h"
#include "ui/dialogs/file_dialog.h"

#include <imgui.h>

#include <cstring>

namespace kuf {

SettingsDialog::SettingsDialog() { pendingConfig_ = config_; }

void SettingsDialog::open() {
	open_ = true;
	pendingConfig_ = config_;
	std::strncpy(crusadersPathBuf_, pendingConfig_.crusadersPath.c_str(),
		     sizeof(crusadersPathBuf_) - 1);
	crusadersPathBuf_[sizeof(crusadersPathBuf_) - 1] = '\0';
	std::strncpy(heroesPathBuf_, pendingConfig_.heroesPath.c_str(),
		     sizeof(heroesPathBuf_) - 1);
	heroesPathBuf_[sizeof(heroesPathBuf_) - 1] = '\0';
}

bool SettingsDialog::draw() {
	if (!open_) return false;

	ImGui::SetNextWindowSize(ImVec2(500, 350), ImGuiCond_FirstUseEver);

	if (!ImGui::Begin("Settings", &open_)) {
		ImGui::End();
		return open_;
	}

	if (ImGui::BeginTabBar("SettingsTabs")) {
		// Appearance tab.
		if (ImGui::BeginTabItem("Appearance")) {
			ImGui::Text("Theme");
			const char *themes[] = {"Dark", "Light", "Classic"};
			int currentTheme =
			    static_cast<int>(pendingConfig_.theme);
			if (ImGui::Combo("##Theme", &currentTheme, themes, 3)) {
				pendingConfig_.theme =
				    static_cast<Theme>(currentTheme);
			}

			ImGui::Spacing();
			ImGui::SliderFloat("Font Size",
					   &pendingConfig_.fontSize, 10.0f,
					   24.0f, "%.0f");

			ImGui::EndTabItem();
		}

		// General tab.
		if (ImGui::BeginTabItem("General")) {
			ImGui::SliderInt("Max Recent Files",
					 &pendingConfig_.maxRecentFiles, 5, 20);

			ImGui::EndTabItem();
		}

		// Games tab.
		if (ImGui::BeginTabItem("Games")) {
			ImGui::Text("Crusaders Path");
			ImGui::SetNextItemWidth(-80.0f);
			ImGui::InputText("##CrusadersPath", crusadersPathBuf_,
					 sizeof(crusadersPathBuf_));
			ImGui::SameLine();
			if (ImGui::Button("Browse##Crusaders")) {
				if (auto path = FileDialog::openFolder()) {
					std::strncpy(
					    crusadersPathBuf_, path->c_str(),
					    sizeof(crusadersPathBuf_) - 1);
					crusadersPathBuf_
					    [sizeof(crusadersPathBuf_) - 1] =
						'\0';
				}
			}

			ImGui::Spacing();

			ImGui::Text("Heroes Path");
			ImGui::SetNextItemWidth(-80.0f);
			ImGui::InputText("##HeroesPath", heroesPathBuf_,
					 sizeof(heroesPathBuf_));
			ImGui::SameLine();
			if (ImGui::Button("Browse##Heroes")) {
				if (auto path = FileDialog::openFolder()) {
					std::strncpy(
					    heroesPathBuf_, path->c_str(),
					    sizeof(heroesPathBuf_) - 1);
					heroesPathBuf_[sizeof(heroesPathBuf_) -
						       1] = '\0';
				}
			}

			ImGui::Spacing();

			if (ImGui::Button("Auto-detect")) {
#ifdef _WIN32
				auto games = detectSteamGames();
				for (const auto &game : games) {
					if (game.name.find("Crusaders") !=
					    std::string::npos) {
						std::strncpy(
						    crusadersPathBuf_,
						    game.path.c_str(),
						    sizeof(crusadersPathBuf_) -
							1);
						crusadersPathBuf_
						    [sizeof(crusadersPathBuf_) -
						     1] = '\0';
					}
					if (game.name.find("Heroes") !=
					    std::string::npos) {
						std::strncpy(
						    heroesPathBuf_,
						    game.path.c_str(),
						    sizeof(heroesPathBuf_) - 1);
						heroesPathBuf_
						    [sizeof(heroesPathBuf_) -
						     1] = '\0';
					}
				}
#endif
			}
#ifndef _WIN32
			ImGui::SameLine();
			ImGui::TextDisabled("(Windows only)");
#endif

			ImGui::EndTabItem();
		}

		ImGui::EndTabBar();
	}

	ImGui::Separator();

	if (ImGui::Button("Apply", ImVec2(80, 0))) {
		pendingConfig_.crusadersPath = crusadersPathBuf_;
		pendingConfig_.heroesPath = heroesPathBuf_;
		config_ = pendingConfig_;
		apply();
		save();
	}
	ImGui::SameLine();
	if (ImGui::Button("OK", ImVec2(80, 0))) {
		pendingConfig_.crusadersPath = crusadersPathBuf_;
		pendingConfig_.heroesPath = heroesPathBuf_;
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
	if (config_.fontSize != appliedFontSize_) {
		appliedFontSize_ = config_.fontSize;
		if (onFontSizeChanged_) {
			onFontSizeChanged_(config_.fontSize);
		}
	}
	if (onGamePathsChanged_) {
		onGamePathsChanged_();
	}
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

void SettingsDialog::applyDarkTheme() { ImGui::StyleColorsDark(); }

void SettingsDialog::applyLightTheme() { ImGui::StyleColorsLight(); }

void SettingsDialog::applyClassicTheme() { ImGui::StyleColorsClassic(); }

void SettingsDialog::load() {
	config_ = loadConfig();
	pendingConfig_ = config_;
}

void SettingsDialog::save() { saveConfig(config_); }

} // namespace kuf
