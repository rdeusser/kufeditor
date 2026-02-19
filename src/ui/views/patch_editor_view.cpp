#include "ui/views/patch_editor_view.h"

#include <imgui.h>

namespace kuf {

PatchEditorView::PatchEditorView() : View("Patch Editor") {
    patches_ = exePatches();
    statuses_.resize(patches_.size(), PatchStatus::Unknown);
}

void PatchEditorView::drawContent() {
    if (gameDirectory_.empty()) {
        ImGui::TextDisabled("Set a game directory to manage exe patches.");
        return;
    }

    if (!std::filesystem::exists(exePath_)) {
        ImGui::TextWrapped("Kuf2Main.exe not found at %s", exePath_.string().c_str());
        return;
    }

    if (!statusLoaded_) {
        refreshStatus();
        statusLoaded_ = true;
    }

    ImGui::Text("Executable: %s", exePath_.string().c_str());
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

    for (size_t i = 0; i < patches_.size(); ++i) {
        const auto& patch = patches_[i];
        auto& status = statuses_[i];

        ImGui::PushID(static_cast<int>(i));

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
                bool ok = enabled ? applyPatch(exePath_, patch) : revertPatch(exePath_, patch);
                if (ok) {
                    status = enabled ? PatchStatus::Applied : PatchStatus::NotApplied;
                }
            }
        }

        ImGui::TextWrapped("%s", patch.description);
        ImGui::Spacing();

        ImGui::PopID();
    }
}

void PatchEditorView::setGameDirectory(const std::string& dir) {
    gameDirectory_ = dir;
    exePath_ = std::filesystem::path(dir) / "Kuf2Main.exe";
    statusLoaded_ = false;
}

void PatchEditorView::refreshStatus() {
    for (size_t i = 0; i < patches_.size(); ++i) {
        statuses_[i] = checkPatch(exePath_, patches_[i]);
    }
}

} // namespace kuf
