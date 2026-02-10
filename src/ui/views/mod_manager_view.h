#pragma once

#include "ui/views/view.h"
#include "core/async_task.h"
#include "mods/backup_manager.h"
#include "mods/mod_manager.h"

#include <functional>
#include <string>
#include <vector>

namespace kuf {

class ModManagerView : public View {
public:
    ModManagerView();

    void drawContent() override;

    void setGameDirectory(const std::string& dir) { gameDirectory_ = dir; }
    void setOnError(std::function<void(const std::string&)> cb) { onError_ = std::move(cb); }

    void restoreLatestBackup();

private:
    void drawBackupsSection();
    void drawModLibrarySection();
    void drawCreateModSection();
    void drawProgressOverlay();
    void refreshBackups();
    void refreshMods();

    std::string gameDirectory_;
    AsyncTask task_;

    // Backups.
    std::vector<BackupInfo> backups_;
    bool backupsLoaded_ = false;

    // Mod library.
    std::vector<ModInfo> mods_;
    bool modsLoaded_ = false;
    int selectedMod_ = -1;

    // Create mod form.
    char modName_[128] = {};
    char modVersion_[32] = "1.0.0";
    char modAuthor_[128] = {};
    char modDescription_[256] = {};
    int modGame_ = 0; // 0 = crusaders, 1 = heroes
    std::vector<std::string> modFiles_;

    // Confirmation popups.
    bool showRestoreConfirm_ = false;
    bool showDeleteConfirm_ = false;
    bool showApplyConfirm_ = false;
    int pendingBackupIndex_ = -1;
    int pendingModIndex_ = -1;

    std::function<void(const std::string&)> onError_;
};

} // namespace kuf
