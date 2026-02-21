#pragma once

#include "patches/exe_patch.h"
#include "ui/views/view.h"

#include <filesystem>
#include <string>
#include <vector>

namespace kuf {

class PatchEditorView : public View {
      public:
	PatchEditorView();

	void drawContent() override;

	void setGameDirectory(const std::string &dir);

      private:
	void refreshStatus();

	std::string gameDirectory_;
	std::filesystem::path exePath_;
	std::vector<ExePatch> patches_;
	std::vector<PatchStatus> statuses_;
	bool statusLoaded_ = false;
};

} // namespace kuf
