#pragma once

#include "core/document.h"
#include "core/name_dictionary.h"

#include <imgui_node_editor.h>

#include <functional>
#include <memory>
#include <string>
#include <vector>

namespace ax {
namespace NodeEditor {
struct EditorContext;
}
} // namespace ax

namespace kuf {

namespace ed = ax::NodeEditor;

class StgNodeGraph {
      public:
	StgNodeGraph();
	~StgNodeGraph();

	StgNodeGraph(const StgNodeGraph &) = delete;
	StgNodeGraph &operator=(const StgNodeGraph &) = delete;

	void initialize(std::shared_ptr<OpenDocument> doc,
			const NameDictionary *nameDict);
	void draw();

      private:
	void autoLayout();

	void drawConditionNode(size_t blockIdx, size_t eventIdx,
			       size_t entryIdx, StgScriptEntry &entry,
			       StgEvent &event);
	void drawActionNode(size_t blockIdx, size_t eventIdx, size_t entryIdx,
			    StgScriptEntry &entry, StgEvent &event);
	void drawEventLabelNode(size_t blockIdx, size_t eventIdx,
				StgEvent &event);
	void drawParamWidgets(StgScriptEntry &entry, bool isCondition,
			      StgEvent &event);
	void drawParamValue(const char *label, StgParamValue &param,
			    StgEvent &event, const char *paramHint);

	void handleContextMenus();
	void renderDeferredPopups();

	struct DeferredPopup {
		std::string id;
		bool shouldOpen = false;
		std::function<void()> render;
	};

	ed::EditorContext *context_ = nullptr;
	std::shared_ptr<OpenDocument> document_;
	const NameDictionary *nameDict_ = nullptr;
	bool needsLayout_ = true;
	std::vector<DeferredPopup> deferredPopups_;

	// Context menu state.
	ed::NodeId contextNodeId_;
	size_t contextBlockIdx_ = 0;
	size_t contextEventIdx_ = 0;
};

} // namespace kuf
