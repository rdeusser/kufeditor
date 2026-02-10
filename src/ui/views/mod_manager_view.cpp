#include "ui/views/mod_manager_view.h"
#include "ui/dialogs/file_dialog.h"

#include <imgui.h>

#include <chrono>
#include <cstring>
#include <filesystem>
#include <iomanip>
#include <sstream>

namespace kuf {

namespace {

std::string formatBytes(size_t bytes) {
    if (bytes < 1024) return std::to_string(bytes) + " B";
    if (bytes < 1024 * 1024) return std::to_string(bytes / 1024) + " KB";
    return std::to_string(bytes / (1024 * 1024)) + " MB";
}

std::string currentIso8601() {
    auto now = std::chrono::system_clock::now();
    auto time = std::chrono::system_clock::to_time_t(now);
    std::tm tm{};
#ifdef _WIN32
    gmtime_s(&tm, &time);
#else
    gmtime_r(&time, &tm);
#endif
    std::ostringstream ss;
    ss << std::put_time(&tm, "%Y-%m-%dT%H:%M:%SZ");
    return ss.str();
}

} // namespace

ModManagerView::ModManagerView() : View("Mod Manager") {}

void ModManagerView::drawContent() {
    if (!installedModsLoaded_) {
        refreshInstalledMods();
        installedModsLoaded_ = true;
    }

    float height = ImGui::GetContentRegionAvail().y;

    ImGui::BeginChild("InstalledModsSidebar", ImVec2(220, height), ImGuiChildFlags_Borders);
    drawInstalledSidebar();
    ImGui::EndChild();

    ImGui::SameLine();

    ImGui::BeginChild("ModManagerContent", ImVec2(0, height), ImGuiChildFlags_Borders);
    drawMainContent();
    ImGui::EndChild();
}

void ModManagerView::drawInstalledSidebar() {
    ImGui::Text("Installed Mods");
    ImGui::Separator();

    if (installedMods_.empty()) {
        ImGui::TextDisabled("No mods installed.");
    } else {
        for (int i = 0; i < static_cast<int>(installedMods_.size()); ++i) {
            bool isSelected = (selectedInstalledMod_ == i);
            if (ImGui::Selectable(installedMods_[i].name.c_str(), isSelected)) {
                selectedInstalledMod_ = isSelected ? -1 : i;
            }
        }
    }

    if (selectedInstalledMod_ >= 0 &&
        selectedInstalledMod_ < static_cast<int>(installedMods_.size())) {
        const auto& mod = installedMods_[selectedInstalledMod_];

        ImGui::Spacing();
        ImGui::Separator();
        ImGui::Spacing();

        ImGui::Text("Version: %s", mod.version.c_str());
        if (!mod.author.empty()) {
            ImGui::Text("Author: %s", mod.author.c_str());
        }
        ImGui::Text("Game: %s", mod.game.c_str());
        if (!mod.installedAt.empty()) {
            ImGui::Text("Installed: %s", mod.installedAt.c_str());
        }

        ImGui::Spacing();
        if (ImGui::Button("Uninstall", ImVec2(-1, 0))) {
            showUninstallConfirm_ = true;
        }
    }

    if (showUninstallConfirm_) {
        ImGui::OpenPopup("Confirm Uninstall");
        showUninstallConfirm_ = false;
    }
    if (ImGui::BeginPopupModal("Confirm Uninstall", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::Text("Remove this mod from the installed list?");
        ImGui::TextDisabled("(Files in the game directory are not reverted.)");
        ImGui::Separator();
        if (ImGui::Button("Uninstall", ImVec2(120, 0))) {
            if (selectedInstalledMod_ >= 0 &&
                selectedInstalledMod_ < static_cast<int>(installedMods_.size())) {
                ModManager::markUninstalled(installedMods_[selectedInstalledMod_].name);
                selectedInstalledMod_ = -1;
                refreshInstalledMods();
            }
            ImGui::CloseCurrentPopup();
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel", ImVec2(120, 0))) {
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }
}

void ModManagerView::drawMainContent() {
    bool taskRunning = task_.state() == AsyncTaskState::Running;

    ImGui::BeginDisabled(taskRunning);

    drawBackupsSection();
    ImGui::Spacing();
    drawModLibrarySection();
    ImGui::Spacing();
    drawCreateModSection();

    ImGui::EndDisabled();

    drawProgressOverlay();

    // Handle async task completion.
    if (task_.state() == AsyncTaskState::Completed) {
        refreshBackups();
        refreshMods();
        refreshInstalledMods();
        task_.reset();
    } else if (task_.state() == AsyncTaskState::Failed) {
        if (onError_) {
            onError_(task_.error());
        }
        task_.reset();
    }
}

void ModManagerView::drawBackupsSection() {
    if (!backupsLoaded_) {
        refreshBackups();
        backupsLoaded_ = true;
    }

    if (ImGui::CollapsingHeader("Backups", ImGuiTreeNodeFlags_DefaultOpen)) {
        bool hasGameDir = !gameDirectory_.empty();

        ImGui::BeginDisabled(!hasGameDir);
        if (ImGui::Button("Create Backup")) {
            std::string dir = gameDirectory_;
            task_.start([dir](AsyncTask& t) {
                return BackupManager::createBackup(dir, t);
            });
        }
        ImGui::EndDisabled();

        if (!hasGameDir) {
            ImGui::SameLine();
            ImGui::TextDisabled("(Set game directory first)");
        }

        if (!backups_.empty()) {
            ImGui::Spacing();

            if (ImGui::BeginTable("BackupsTable", 5, ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg)) {
                ImGui::TableSetupColumn("Timestamp", ImGuiTableColumnFlags_WidthStretch);
                ImGui::TableSetupColumn("Files", ImGuiTableColumnFlags_WidthFixed, 60.0f);
                ImGui::TableSetupColumn("Size", ImGuiTableColumnFlags_WidthFixed, 80.0f);
                ImGui::TableSetupColumn("##Restore", ImGuiTableColumnFlags_WidthFixed, 60.0f);
                ImGui::TableSetupColumn("##Delete", ImGuiTableColumnFlags_WidthFixed, 60.0f);
                ImGui::TableHeadersRow();

                for (int i = 0; i < static_cast<int>(backups_.size()); ++i) {
                    const auto& backup = backups_[i];
                    ImGui::TableNextRow();

                    ImGui::TableNextColumn();
                    ImGui::Text("%s", backup.timestamp.c_str());

                    ImGui::TableNextColumn();
                    ImGui::Text("%zu", backup.fileCount);

                    ImGui::TableNextColumn();
                    ImGui::Text("%s", formatBytes(backup.totalBytes).c_str());

                    ImGui::TableNextColumn();
                    ImGui::PushID(i);
                    ImGui::BeginDisabled(!hasGameDir);
                    if (ImGui::SmallButton("Restore")) {
                        pendingBackupIndex_ = i;
                        showRestoreConfirm_ = true;
                    }
                    ImGui::EndDisabled();

                    ImGui::TableNextColumn();
                    if (ImGui::SmallButton("Delete")) {
                        pendingBackupIndex_ = i;
                        showDeleteConfirm_ = true;
                    }
                    ImGui::PopID();
                }

                ImGui::EndTable();
            }
        } else {
            ImGui::TextDisabled("No backups found.");
        }
    }

    // Restore confirmation popup.
    if (showRestoreConfirm_) {
        ImGui::OpenPopup("Confirm Restore");
        showRestoreConfirm_ = false;
    }
    if (ImGui::BeginPopupModal("Confirm Restore", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::Text("Restore this backup? This will overwrite files in the game directory.");
        ImGui::Separator();
        if (ImGui::Button("Restore", ImVec2(120, 0))) {
            if (pendingBackupIndex_ >= 0 && pendingBackupIndex_ < static_cast<int>(backups_.size())) {
                auto backup = backups_[pendingBackupIndex_];
                std::string dir = gameDirectory_;
                task_.start([backup, dir](AsyncTask& t) {
                    return BackupManager::restoreBackup(backup, dir, t);
                });
            }
            ImGui::CloseCurrentPopup();
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel", ImVec2(120, 0))) {
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }

    // Delete confirmation popup.
    if (showDeleteConfirm_) {
        ImGui::OpenPopup("Confirm Delete Backup");
        showDeleteConfirm_ = false;
    }
    if (ImGui::BeginPopupModal("Confirm Delete Backup", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::Text("Permanently delete this backup?");
        ImGui::Separator();
        if (ImGui::Button("Delete", ImVec2(120, 0))) {
            if (pendingBackupIndex_ >= 0 && pendingBackupIndex_ < static_cast<int>(backups_.size())) {
                BackupManager::deleteBackup(backups_[pendingBackupIndex_]);
                refreshBackups();
            }
            ImGui::CloseCurrentPopup();
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel", ImVec2(120, 0))) {
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }
}

void ModManagerView::drawModLibrarySection() {
    if (!modsLoaded_) {
        refreshMods();
        modsLoaded_ = true;
    }

    if (ImGui::CollapsingHeader("Mod Library", ImGuiTreeNodeFlags_DefaultOpen)) {
        if (ImGui::Button("Import Mod (.zip)")) {
            if (auto path = FileDialog::openFile("*.zip")) {
                auto result = ModManager::importMod(*path);
                if (std::holds_alternative<ModInfo>(result)) {
                    refreshMods();
                } else {
                    if (onError_) {
                        onError_(std::get<std::string>(result));
                    }
                }
            }
        }

        if (!mods_.empty()) {
            ImGui::Spacing();

            if (ImGui::BeginTable("ModsTable", 6, ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg)) {
                ImGui::TableSetupColumn("Name", ImGuiTableColumnFlags_WidthStretch);
                ImGui::TableSetupColumn("Version", ImGuiTableColumnFlags_WidthFixed, 60.0f);
                ImGui::TableSetupColumn("Author", ImGuiTableColumnFlags_WidthFixed, 100.0f);
                ImGui::TableSetupColumn("Game", ImGuiTableColumnFlags_WidthFixed, 80.0f);
                ImGui::TableSetupColumn("##Apply", ImGuiTableColumnFlags_WidthFixed, 50.0f);
                ImGui::TableSetupColumn("##Remove", ImGuiTableColumnFlags_WidthFixed, 60.0f);
                ImGui::TableHeadersRow();

                for (int i = 0; i < static_cast<int>(mods_.size()); ++i) {
                    const auto& mod = mods_[i];
                    ImGui::TableNextRow();

                    ImGui::TableNextColumn();
                    bool isSelected = (selectedMod_ == i);
                    if (ImGui::Selectable(mod.metadata.name.c_str(), isSelected,
                                          ImGuiSelectableFlags_SpanAllColumns)) {
                        selectedMod_ = isSelected ? -1 : i;
                    }

                    ImGui::TableNextColumn();
                    ImGui::Text("%s", mod.metadata.version.c_str());

                    ImGui::TableNextColumn();
                    ImGui::Text("%s", mod.metadata.author.c_str());

                    ImGui::TableNextColumn();
                    ImGui::Text("%s", mod.metadata.game.c_str());

                    ImGui::TableNextColumn();
                    ImGui::PushID(i);
                    bool hasGameDir = !gameDirectory_.empty();
                    ImGui::BeginDisabled(!hasGameDir);
                    if (ImGui::SmallButton("Apply")) {
                        pendingModIndex_ = i;
                        showApplyConfirm_ = true;
                    }
                    ImGui::EndDisabled();

                    ImGui::TableNextColumn();
                    if (ImGui::SmallButton("Remove")) {
                        ModManager::removeMod(mod);
                        refreshMods();
                        if (selectedMod_ >= static_cast<int>(mods_.size())) {
                            selectedMod_ = -1;
                        }
                    }
                    ImGui::PopID();
                }

                ImGui::EndTable();
            }

            // Show selected mod description.
            if (selectedMod_ >= 0 && selectedMod_ < static_cast<int>(mods_.size())) {
                const auto& mod = mods_[selectedMod_];
                if (!mod.metadata.description.empty()) {
                    ImGui::Spacing();
                    ImGui::TextWrapped("%s", mod.metadata.description.c_str());
                }
                if (!mod.metadata.files.empty()) {
                    ImGui::Spacing();
                    ImGui::Text("Files (%zu):", mod.metadata.files.size());
                    for (const auto& f : mod.metadata.files) {
                        ImGui::BulletText("%s", f.c_str());
                    }
                }
            }
        } else {
            ImGui::TextDisabled("No mods imported.");
        }
    }

    // Apply confirmation popup.
    if (showApplyConfirm_) {
        ImGui::OpenPopup("Confirm Apply Mod");
        showApplyConfirm_ = false;
    }
    if (ImGui::BeginPopupModal("Confirm Apply Mod", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::Text("Apply this mod? Files in the game directory will be overwritten.");
        ImGui::Separator();
        if (ImGui::Button("Apply", ImVec2(120, 0))) {
            if (pendingModIndex_ >= 0 && pendingModIndex_ < static_cast<int>(mods_.size())) {
                auto mod = mods_[pendingModIndex_];
                std::string dir = gameDirectory_;
                task_.start([mod, dir](AsyncTask& t) {
                    bool ok = ModManager::applyMod(mod, dir, t);
                    if (ok) {
                        ModManager::markInstalled(mod);
                    }
                    return ok;
                });
            }
            ImGui::CloseCurrentPopup();
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel", ImVec2(120, 0))) {
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }
}

void ModManagerView::drawCreateModSection() {
    if (ImGui::CollapsingHeader("Create Mod")) {
        ImGui::InputText("Name", modName_, sizeof(modName_));
        ImGui::InputText("Version", modVersion_, sizeof(modVersion_));
        ImGui::InputText("Author", modAuthor_, sizeof(modAuthor_));
        ImGui::InputTextMultiline("Description", modDescription_, sizeof(modDescription_),
                                   ImVec2(-1, 60));

        const char* gameOptions[] = {"crusaders", "heroes"};
        ImGui::Combo("Game", &modGame_, gameOptions, 2);

        ImGui::Spacing();
        ImGui::Text("Files:");

        bool hasGameDir = !gameDirectory_.empty();
        ImGui::BeginDisabled(!hasGameDir);
        if (ImGui::Button("Add File...")) {
            if (auto path = FileDialog::openFile("*", gameDirectory_.c_str())) {
                namespace fs = std::filesystem;
                // Convert absolute path to relative path from game directory.
                auto relPath = fs::relative(*path, gameDirectory_);
                std::string relStr = relPath.generic_string();

                // Check for duplicates.
                bool duplicate = false;
                for (const auto& f : modFiles_) {
                    if (f == relStr) {
                        duplicate = true;
                        break;
                    }
                }
                if (!duplicate) {
                    modFiles_.push_back(relStr);
                }
            }
        }
        ImGui::EndDisabled();

        if (!hasGameDir) {
            ImGui::SameLine();
            ImGui::TextDisabled("(Set game directory first)");
        }

        // File list with remove buttons.
        int removeIndex = -1;
        for (int i = 0; i < static_cast<int>(modFiles_.size()); ++i) {
            ImGui::PushID(i);
            if (ImGui::SmallButton("X")) {
                removeIndex = i;
            }
            ImGui::SameLine();
            ImGui::Text("%s", modFiles_[i].c_str());
            ImGui::PopID();
        }
        if (removeIndex >= 0) {
            modFiles_.erase(modFiles_.begin() + removeIndex);
        }

        ImGui::Spacing();
        bool canExport = modName_[0] != '\0' && modVersion_[0] != '\0' &&
                         !modFiles_.empty() && hasGameDir;
        ImGui::BeginDisabled(!canExport);
        if (ImGui::Button("Export Mod (.zip)")) {
            std::string defaultName = std::string(modName_) + ".zip";
            if (auto savePath = FileDialog::saveFile("*.zip", defaultName.c_str())) {
                ModMetadata meta;
                meta.name = modName_;
                meta.version = modVersion_;
                meta.author = modAuthor_;
                meta.description = modDescription_;
                meta.game = gameOptions[modGame_];
                meta.created = currentIso8601();
                meta.files = modFiles_;

                std::string dir = gameDirectory_;
                auto files = modFiles_;
                std::string outPath = *savePath;
                task_.start([meta, dir, files, outPath](AsyncTask& t) {
                    return ModManager::createMod(meta, dir, files, outPath, t);
                });
            }
        }
        ImGui::EndDisabled();
    }
}

void ModManagerView::drawProgressOverlay() {
    if (task_.state() != AsyncTaskState::Running) return;

    float windowHeight = ImGui::GetWindowHeight();
    float barHeight = 40.0f;
    ImGui::SetCursorPosY(windowHeight - barHeight - 8.0f);

    ImVec4 overlayColor(0.0f, 0.0f, 0.0f, 0.6f);
    ImGui::PushStyleColor(ImGuiCol_ChildBg, overlayColor);
    ImGui::BeginChild("ProgressOverlay", ImVec2(-1, barHeight), true);

    float progress = task_.progress();
    std::string status = task_.status();

    ImGui::ProgressBar(progress, ImVec2(-1, 0), status.empty() ? nullptr : status.c_str());

    ImGui::EndChild();
    ImGui::PopStyleColor();
}

void ModManagerView::restoreLatestBackup() {
    if (gameDirectory_.empty()) return;
    if (task_.state() == AsyncTaskState::Running) return;

    auto latest = BackupManager::latestBackup();
    if (!latest) {
        if (onError_) {
            onError_("No backups found");
        }
        return;
    }

    auto backup = *latest;
    std::string dir = gameDirectory_;
    task_.start([backup, dir](AsyncTask& t) {
        return BackupManager::restoreBackup(backup, dir, t);
    });
}

void ModManagerView::refreshBackups() {
    backups_ = BackupManager::listBackups();
}

void ModManagerView::refreshMods() {
    mods_ = ModManager::listMods();
    if (selectedMod_ >= static_cast<int>(mods_.size())) {
        selectedMod_ = -1;
    }
}

void ModManagerView::refreshInstalledMods() {
    installedMods_ = ModManager::listInstalledMods();
    if (selectedInstalledMod_ >= static_cast<int>(installedMods_.size())) {
        selectedInstalledMod_ = -1;
    }
}

} // namespace kuf
