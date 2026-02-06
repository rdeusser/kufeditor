#include "core/tab_manager.h"
#include "ui/tabs/troop_editor_tab.h"
#include "ui/tabs/text_editor_tab.h"
#include "formats/sox_binary.h"
#include "formats/sox_text.h"
#include "formats/sox_encoding.h"

#include <filesystem>
#include <fstream>

namespace kuf {

namespace {

std::string getFileName(const std::string& path) {
    auto pos = path.find_last_of("/\\");
    if (pos != std::string::npos) {
        return path.substr(pos + 1);
    }
    return path;
}

} // namespace

OpenFileResult TabManager::openFile(const std::string& path) {
    // Check if file is already open.
    if (auto* existing = findTabByPath(path)) {
        activeTab_ = existing;
        return {existing, OpenResult::Success};
    }

    auto doc = loadDocument(path);
    if (!doc) {
        return {nullptr, OpenResult::FileNotFound};
    }

    auto* tab = createTabForDocument(std::move(doc));
    if (tab) {
        activeTab_ = tab;
        return {tab, OpenResult::Success};
    }

    return {nullptr, OpenResult::UnsupportedFormat};
}

void TabManager::closeTab(EditorTab* tab) {
    if (!tab) return;

    auto it = std::find_if(tabs_.begin(), tabs_.end(),
        [tab](const auto& t) { return t.get() == tab; });

    if (it != tabs_.end()) {
        if (activeTab_ == tab) {
            activeTab_ = nullptr;
            // Try to select adjacent tab.
            if (it + 1 != tabs_.end()) {
                activeTab_ = (it + 1)->get();
            } else if (it != tabs_.begin()) {
                activeTab_ = (it - 1)->get();
            }
        }
        tabs_.erase(it);
    }
}

void TabManager::saveDocument(OpenDocument* doc) {
    if (!doc || doc->path.empty()) return;

    std::vector<std::byte> data;
    if (doc->binaryData) {
        data = doc->binaryData->save();
    } else if (doc->textData) {
        data = doc->textData->save();
    }

    if (!data.empty()) {
        // Re-encode to ASCII hex if the original file was hex-encoded.
        if (doc->isSoxEncoded) {
            data = soxEncode(data);
        }

        std::ofstream file(doc->path, std::ios::binary);
        file.write(reinterpret_cast<const char*>(data.data()),
                   static_cast<std::streamsize>(data.size()));
        doc->dirty = false;
    }
}

void TabManager::saveAll() {
    for (const auto& tab : tabs_) {
        if (tab->document() && tab->document()->dirty) {
            saveDocument(tab->document().get());
        }
    }
}

std::shared_ptr<OpenDocument> TabManager::loadDocument(const std::string& path) {
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    if (!file) return nullptr;

    auto size = file.tellg();
    file.seekg(0);

    auto doc = std::make_shared<OpenDocument>();
    doc->path = path;
    doc->filename = getFileName(path);
    doc->rawData.resize(size);
    file.read(reinterpret_cast<char*>(doc->rawData.data()), size);

    // SOX files use ASCII hex encoding. Decode if detected.
    std::span<const std::byte> parseData = doc->rawData;
    std::vector<std::byte> decodedData;
    doc->isSoxEncoded = isSoxEncoded(doc->rawData);

    if (doc->isSoxEncoded) {
        auto decoded = soxDecode(doc->rawData);
        if (decoded) {
            decodedData = std::move(*decoded);
            parseData = decodedData;
        }
    }

    // Try binary SOX first (has version header = 100).
    auto binary = std::make_shared<SoxBinary>();
    if (binary->load(parseData)) {
        doc->binaryData = binary;
        doc->undoStack->setOnChange([doc = doc.get()]() {
            doc->dirty = true;
        });

        if (onDocumentOpened_) {
            onDocumentOpened_(doc.get());
        }
        return doc;
    }

    // Try text SOX.
    auto text = std::make_shared<SoxText>();
    if (text->load(parseData)) {
        doc->textData = text;
        doc->undoStack->setOnChange([doc = doc.get()]() {
            doc->dirty = true;
        });

        if (onDocumentOpened_) {
            onDocumentOpened_(doc.get());
        }
        return doc;
    }

    // Unknown format - still return doc so we know the file exists.
    if (onDocumentOpened_) {
        onDocumentOpened_(doc.get());
    }
    return doc;
}

EditorTab* TabManager::findTabByPath(const std::string& path) const {
    for (const auto& tab : tabs_) {
        if (tab->document() && tab->document()->path == path) {
            return tab.get();
        }
    }
    return nullptr;
}

EditorTab* TabManager::createTabForDocument(std::shared_ptr<OpenDocument> doc) {
    if (!doc) return nullptr;

    std::unique_ptr<EditorTab> tab;

    if (doc->binaryData) {
        tab = std::make_unique<TroopEditorTab>(std::move(doc));
    } else if (doc->textData) {
        tab = std::make_unique<TextEditorTab>(std::move(doc));
    } else {
        // Unknown format - don't create a tab.
        return nullptr;
    }

    auto* ptr = tab.get();
    tabs_.push_back(std::move(tab));
    return ptr;
}

} // namespace kuf
