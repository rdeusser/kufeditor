#pragma once

#include "core/document.h"

#include <imgui.h>

#include <atomic>
#include <memory>
#include <string>

namespace kuf {

/// Base class for file-backed editor tabs.
class EditorTab {
public:
    explicit EditorTab(std::shared_ptr<OpenDocument> doc)
        : document_(std::move(doc)), tabId_(nextTabId_++) {}
    virtual ~EditorTab() = default;

    virtual void drawContent() = 0;

    std::string tabTitle() const {
        if (!document_) return "Untitled";
        std::string title = document_->filename;
        if (document_->dirty) title += "*";
        return title;
    }

    std::shared_ptr<OpenDocument> document() { return document_; }
    const std::shared_ptr<OpenDocument>& document() const { return document_; }

    bool& isOpen() { return open_; }
    int tabId() const { return tabId_; }

protected:
    std::shared_ptr<OpenDocument> document_;
    bool open_ = true;
    int tabId_;

private:
    static inline std::atomic<int> nextTabId_{1};
};

} // namespace kuf
