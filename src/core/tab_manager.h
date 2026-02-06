#pragma once

#include "core/document.h"
#include "ui/tabs/editor_tab.h"

#include <functional>
#include <memory>
#include <string>
#include <vector>

namespace kuf {

/// Result of attempting to open a file.
enum class OpenResult {
    Success,
    FileNotFound,
    UnsupportedFormat
};

/// Contains the result of openFile() including the tab pointer and status.
struct OpenFileResult {
    EditorTab* tab = nullptr;
    OpenResult result = OpenResult::Success;
};

/// Manages open editor tabs.
class TabManager {
public:
    using OnDocumentOpenedCallback = std::function<void(OpenDocument*)>;

    OpenFileResult openFile(const std::string& path);
    void closeTab(EditorTab* tab);
    void saveDocument(OpenDocument* doc);
    void saveAll();

    EditorTab* activeTab() const { return activeTab_; }
    void setActiveTab(EditorTab* tab) { activeTab_ = tab; }

    const std::vector<std::unique_ptr<EditorTab>>& tabs() const { return tabs_; }

    void setOnDocumentOpened(OnDocumentOpenedCallback cb) {
        onDocumentOpened_ = std::move(cb);
    }

private:
    std::shared_ptr<OpenDocument> loadDocument(const std::string& path);
    EditorTab* findTabByPath(const std::string& path) const;
    EditorTab* createTabForDocument(std::shared_ptr<OpenDocument> doc);

    std::vector<std::unique_ptr<EditorTab>> tabs_;
    EditorTab* activeTab_ = nullptr;
    OnDocumentOpenedCallback onDocumentOpened_;
};

} // namespace kuf
