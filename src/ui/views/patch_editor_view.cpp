#include "ui/views/patch_editor_view.h"

#include <imgui.h>

namespace kuf {

PatchEditorView::PatchEditorView() : View("Patch Manager") {
	patches_ = exePatches();
	statuses_.resize(patches_.size(), PatchStatus::Unknown);
}

void PatchEditorView::setActiveGame(ActiveGame game) {
	activeGame_ = game;
	statusLoaded_ = false;
}

void PatchEditorView::drawContent() {
	if (activeGame_ == ActiveGame::None) {
		ImGui::TextDisabled(
		    "Select a game from the status bar to manage patches.");
		return;
	}

	if (activeGame_ == ActiveGame::Heroes) {
		ImGui::TextDisabled("No patches available for Heroes yet.");
		return;
	}

	if (gameDirectory_.empty()) {
		ImGui::TextDisabled(
		    "Set the Crusaders path in Settings > Games.");
		return;
	}

	if (!std::filesystem::exists(exePath_)) {
		ImGui::TextWrapped("Kuf2Main.exe not found at %s",
				   exePath_.string().c_str());
		return;
	}

	if (!statusLoaded_) {
		refreshStatus();
		statusLoaded_ = true;
	}

	ImGui::Indent(8.0f);

	ImGui::TextDisabled("Executable: %s", exePath_.string().c_str());
	ImGui::Spacing();
	ImGui::Separator();
	ImGui::Spacing();

	// Debug Menu patch (index 0).
	{
		const auto &patch = patches_[0];
		auto &status = statuses_[0];

		ImGui::PushID(0);

		bool enabled = (status == PatchStatus::Applied);
		bool unknown = (status == PatchStatus::Unknown);

		if (unknown) {
			ImGui::BeginDisabled();
			bool dummy = false;
			ImGui::Checkbox(patch.name, &dummy);
			ImGui::EndDisabled();
			ImGui::SameLine();
			ImGui::TextDisabled("(unrecognized exe)");
		} else {
			if (ImGui::Checkbox(patch.name, &enabled)) {
				bool ok = enabled
					      ? applyPatch(exePath_, patch)
					      : revertPatch(exePath_, patch);
				if (ok) {
					status = enabled
						     ? PatchStatus::Applied
						     : PatchStatus::NotApplied;
				}
			}
		}

		ImGui::TextWrapped("%s", patch.description);
		ImGui::Spacing();

		ImGui::PopID();
	}

	ImGui::SeparatorText("Experimental");
	ImGui::Spacing();

	// Terrain Bounds Check patch (index 1).
	{
		const auto &patch = patches_[1];
		auto &status = statuses_[1];

		ImGui::PushID(1);

		bool enabled = (status == PatchStatus::Applied);
		bool unknown = (status == PatchStatus::Unknown);

		if (unknown) {
			ImGui::BeginDisabled();
			bool dummy = false;
			ImGui::Checkbox(patch.name, &dummy);
			ImGui::EndDisabled();
			ImGui::SameLine();
			ImGui::TextDisabled("(unrecognized exe)");
		} else {
			if (ImGui::Checkbox(patch.name, &enabled)) {
				bool ok = enabled
					      ? applyPatch(exePath_, patch)
					      : revertPatch(exePath_, patch);
				if (ok) {
					status = enabled
						     ? PatchStatus::Applied
						     : PatchStatus::NotApplied;
				}
			}
		}

		ImGui::TextWrapped("%s", patch.description);
		ImGui::Spacing();

		ImGui::PopID();
	}

	// Fire Rate preset patch.
	ImGui::SeparatorText("Fire Rate");

	auto presets = fireRatePresets();

	if (fireRateStatus_ == FireRateStatus::Unknown) {
		ImGui::BeginDisabled();
		int dummy = 0;
		ImGui::Combo("Preset##FireRate", &dummy, "Original\0");
		ImGui::EndDisabled();
		ImGui::SameLine();
		ImGui::TextDisabled("(unrecognized exe)");
	} else if (fireRateStatus_ == FireRateStatus::Custom) {
		ImGui::BeginDisabled();
		if (ImGui::BeginCombo("Preset##FireRate", "Custom")) {
			ImGui::EndCombo();
		}
		ImGui::EndDisabled();
		auto values = readFireRateValues(exePath_);
		ImGui::TextWrapped("Custom values: delay=%d, multiplier=x%d, "
				   "factor=%.6f",
				   values.baseDelay, values.multiplier,
				   values.distanceFactor);
	} else {
		if (ImGui::Combo("Preset##FireRate", &fireRatePresetIndex_,
				 "Original\0Fast\0Rapid\0Turbo\0")) {
			if (applyFireRate(
				exePath_,
				presets[fireRatePresetIndex_].values)) {
				fireRateStatus_ = static_cast<FireRateStatus>(
				    fireRatePresetIndex_);
			}
		}
		if (fireRatePresetIndex_ >= 0 &&
		    fireRatePresetIndex_ < static_cast<int>(presets.size())) {
			ImGui::TextWrapped(
			    "%s", presets[fireRatePresetIndex_].description);
		}
	}
	ImGui::Spacing();

	ImGui::Unindent(8.0f);
}

void PatchEditorView::setGameDirectory(const std::string &dir) {
	gameDirectory_ = dir;
	exePath_ = std::filesystem::path(dir) / "Kuf2Main.exe";
	statusLoaded_ = false;
}

void PatchEditorView::refreshStatus() {
	for (size_t i = 0; i < patches_.size(); ++i) {
		statuses_[i] = checkPatch(exePath_, patches_[i]);
	}

	fireRateStatus_ = checkFireRate(exePath_);
	if (fireRateStatus_ == FireRateStatus::Custom ||
	    fireRateStatus_ == FireRateStatus::Unknown) {
		fireRatePresetIndex_ = -1;
	} else {
		fireRatePresetIndex_ = static_cast<int>(fireRateStatus_);
	}
}

} // namespace kuf
