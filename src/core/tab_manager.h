#pragma once

#include "core/document.h"
#include "formats/sox_encoding.h"
#include "ui/tabs/editor_tab.h"

#include <fstream>
#include <functional>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <utility>
#include <vector>

namespace kuf {

namespace tab_manager_detail {

template <typename File> bool finishFileWrite(File &file) {
	file.close();
	return !file.fail();
}

} // namespace tab_manager_detail

/// Result of attempting to open a file.
enum class OpenResult { Success, FileNotFound, UnsupportedFormat };

/// Contains the result of openFile() including the tab pointer and status.
struct OpenFileResult {
	EditorTab *tab = nullptr;
	OpenResult result = OpenResult::Success;
};

enum class SaveStatus {
	Saved,
	InvalidDocument,
	NoData,
	SerializationFailed,
	WriteFailed
};

struct SaveResult {
	SaveStatus status = SaveStatus::InvalidDocument;
	std::string message;
	std::optional<STGSaveError> STGError;
};

/// Manages open editor tabs.
class TabManager {
      public:
	using OnDocumentOpenedCallback = std::function<void(OpenDocument *)>;
	using OnSaveErrorCallback = std::function<void(const SaveResult &)>;
	using FileWriter = std::function<bool(const std::string &,
					      std::span<const std::byte>)>;

	explicit TabManager(FileWriter fileWriter = {})
	    : fileWriter_(std::move(fileWriter)) {}

	OpenFileResult openFile(const std::string &path);
	void closeTab(EditorTab *tab);
	SaveResult saveDocument(OpenDocument *doc);
	void saveAll();

	EditorTab *activeTab() const { return activeTab_; }
	void setActiveTab(EditorTab *tab) { activeTab_ = tab; }

	const std::vector<std::unique_ptr<EditorTab>> &tabs() const {
		return tabs_;
	}

	void setOnDocumentOpened(OnDocumentOpenedCallback cb) {
		onDocumentOpened_ = std::move(cb);
	}

	void setOnSaveError(OnSaveErrorCallback cb) {
		onSaveError_ = std::move(cb);
	}

      private:
	std::shared_ptr<OpenDocument> loadDocument(const std::string &path);
	EditorTab *findTabByPath(const std::string &path) const;
	EditorTab *createTabForDocument(std::shared_ptr<OpenDocument> doc);

	std::vector<std::unique_ptr<EditorTab>> tabs_;
	EditorTab *activeTab_ = nullptr;
	OnDocumentOpenedCallback onDocumentOpened_;
	OnSaveErrorCallback onSaveError_;
	FileWriter fileWriter_;
};

inline SaveResult TabManager::saveDocument(OpenDocument *doc) {
	if (!doc || doc->path.empty()) {
		return {SaveStatus::InvalidDocument, "No document path",
			std::nullopt};
	}

	std::vector<std::byte> data;
	if (doc->binaryData) {
		data = doc->binaryData->save();
	} else if (doc->skillData) {
		data = doc->skillData->save();
	} else if (doc->textData) {
		data = doc->textData->save();
	} else if (doc->stgData) {
		auto result = doc->stgData->trySave();
		if (!result.succeeded()) {
			SaveResult failure{SaveStatus::SerializationFailed,
					   result.error->message, result.error};
			if (onSaveError_) onSaveError_(failure);
			return failure;
		}
		data = std::move(result.bytes);
	} else if (doc->saveData) {
		data = doc->saveData->save();
	}

	if (data.empty()) {
		return {SaveStatus::NoData, "Document produced no data",
			std::nullopt};
	}

	if (doc->isSoxEncoded) {
		data = soxEncode(data);
	}

	bool wrote = false;
	if (fileWriter_) {
		wrote = fileWriter_(doc->path, data);
	} else {
		std::ofstream file(doc->path,
				   std::ios::binary | std::ios::trunc);
		if (file) {
			file.write(reinterpret_cast<const char *>(data.data()),
				   static_cast<std::streamsize>(data.size()));
			const bool writeSucceeded = file.good();
			const bool closeSucceeded =
			    tab_manager_detail::finishFileWrite(file);
			wrote = writeSucceeded && closeSucceeded;
		}
	}

	if (!wrote) {
		SaveResult failure{SaveStatus::WriteFailed,
				   "Could not write " + doc->path,
				   std::nullopt};
		if (onSaveError_) onSaveError_(failure);
		return failure;
	}

	doc->dirty = false;
	return {SaveStatus::Saved, {}, std::nullopt};
}

} // namespace kuf
