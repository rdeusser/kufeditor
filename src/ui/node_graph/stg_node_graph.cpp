#include "ui/node_graph/stg_node_graph.h"

#include <imgui.h>

#include <algorithm>
#include <cstdio>
#include <cstring>

#include "formats/stg_script_catalog.h"
#include "ui/node_graph/node_id_scheme.h"
#include "ui/stg_display_helpers.h"

namespace kuf {

namespace ed = ax::NodeEditor;

static ImColor eventColor(size_t eventIdx) {
	constexpr ImColor palette[] = {
	    ImColor(100, 160, 255, 200), ImColor(255, 160, 100, 200),
	    ImColor(100, 255, 160, 200), ImColor(255, 100, 160, 200),
	    ImColor(160, 100, 255, 200), ImColor(255, 255, 100, 200),
	    ImColor(100, 255, 255, 200), ImColor(255, 100, 255, 200),
	};
	return palette[eventIdx % 8];
}

StgNodeGraph::StgNodeGraph() = default;

StgNodeGraph::~StgNodeGraph() {
	if (context_) {
		ed::DestroyEditor(context_);
	}
}

void StgNodeGraph::initialize(std::shared_ptr<OpenDocument> doc,
			      const NameDictionary *nameDict) {
	document_ = std::move(doc);
	nameDict_ = nameDict;
	needsLayout_ = true;

	if (context_) {
		ed::DestroyEditor(context_);
	}

	ed::Config config;
	config.SettingsFile = nullptr;
	context_ = ed::CreateEditor(&config);
}

void StgNodeGraph::draw() {
	if (!context_ || !document_ || !document_->stgData) return;

	ed::SetCurrentEditor(context_);

	if (needsLayout_) {
		autoLayout();
		needsLayout_ = false;
	}

	ed::Begin("StgNodeGraph");

	auto &blocks = document_->stgData->eventBlocks();
	deferredPopups_.clear();

	for (size_t b = 0; b < blocks.size(); ++b) {
		auto &block = blocks[b];
		for (size_t e = 0; e < block.events.size(); ++e) {
			auto &event = block.events[e];

			drawEventLabelNode(b, e, event);

			for (size_t c = 0; c < event.conditions.size(); ++c) {
				drawConditionNode(b, e, c, event.conditions[c],
						  event);
			}

			for (size_t a = 0; a < event.actions.size(); ++a) {
				drawActionNode(b, e, a, event.actions[a],
					       event);
			}

			// Draw links: every condition → every action.
			ImColor linkColor = eventColor(e);
			for (size_t c = 0; c < event.conditions.size(); ++c) {
				for (size_t a = 0; a < event.actions.size();
				     ++a) {
					ed::Link(
					    ed::LinkId(eventLink(b, e, c, a)),
					    ed::PinId(
						conditionOutputPin(b, e, c)),
					    ed::PinId(actionInputPin(b, e, a)),
					    linkColor, 2.0f);
				}
			}
		}
	}

	handleContextMenus();

	ed::End();

	// Deferred popups rendered outside ed::Begin/End.
	renderDeferredPopups();

	ed::SetCurrentEditor(nullptr);
}

void StgNodeGraph::drawEventLabelNode(size_t blockIdx, size_t eventIdx,
				      StgEvent &event) {
	uint64_t nodeId = eventGroupNode(blockIdx, eventIdx);

	ed::PushStyleColor(ed::StyleColor_NodeBg, ImColor(40, 40, 40, 230));
	ed::PushStyleColor(ed::StyleColor_NodeBorder, ImColor(80, 80, 80));

	ed::BeginNode(ed::NodeId(nodeId));
	ImGui::PushID(static_cast<int>(nodeId >> 32));
	ImGui::PushID(static_cast<int>(nodeId & 0xFFFFFFFF));

	std::string displayDesc;
	if (nameDict_) {
		displayDesc = nameDict_->translate(event.description);
	}
	if (displayDesc.empty()) displayDesc = event.description;

	char header[128];
	snprintf(header, sizeof(header), "Event %zu [ID %u] - %s", eventIdx,
		 event.eventId, displayDesc.c_str());
	ImGui::TextUnformatted(header);

	ImGui::PopID();
	ImGui::PopID();
	ed::EndNode();

	ed::PopStyleColor(2);
}

void StgNodeGraph::drawConditionNode(size_t blockIdx, size_t eventIdx,
				     size_t entryIdx, StgScriptEntry &entry,
				     StgEvent &event) {
	uint64_t nodeId = conditionNode(blockIdx, eventIdx, entryIdx);

	ed::PushStyleColor(ed::StyleColor_NodeBg, ImColor(25, 50, 100, 230));
	ed::PushStyleColor(ed::StyleColor_NodeBorder,
			   ImColor(60, 100, 160, 230));

	ed::BeginNode(ed::NodeId(nodeId));
	ImGui::PushID(static_cast<int>(nodeId >> 32));
	ImGui::PushID(static_cast<int>(nodeId & 0xFFFFFFFF));

	const ScriptEntryInfo *info = findConditionInfo(entry.typeId);
	const char *name = info ? info->name : "Unknown";

	ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(0.7f, 0.85f, 1.0f, 1.0f));
	ImGui::Text("%s", name);
	ImGui::PopStyleColor();
	ImGui::Separator();

	drawParamWidgets(entry, true, event);

	// Output pin on the right.
	ed::BeginPin(
	    ed::PinId(conditionOutputPin(blockIdx, eventIdx, entryIdx)),
	    ed::PinKind::Output);
	ImGui::TextUnformatted(">>>");
	ed::EndPin();

	ImGui::PopID();
	ImGui::PopID();
	ed::EndNode();

	ed::PopStyleColor(2);
}

void StgNodeGraph::drawActionNode(size_t blockIdx, size_t eventIdx,
				  size_t entryIdx, StgScriptEntry &entry,
				  StgEvent &event) {
	uint64_t nodeId = actionNode(blockIdx, eventIdx, entryIdx);

	ed::PushStyleColor(ed::StyleColor_NodeBg, ImColor(100, 65, 25, 230));
	ed::PushStyleColor(ed::StyleColor_NodeBorder,
			   ImColor(160, 100, 40, 230));

	ed::BeginNode(ed::NodeId(nodeId));
	ImGui::PushID(static_cast<int>(nodeId >> 32));
	ImGui::PushID(static_cast<int>(nodeId & 0xFFFFFFFF));

	// Input pin on the left.
	ed::BeginPin(ed::PinId(actionInputPin(blockIdx, eventIdx, entryIdx)),
		     ed::PinKind::Input);
	ImGui::TextUnformatted(">>>");
	ed::EndPin();
	ImGui::SameLine();

	const ScriptEntryInfo *info = findActionInfo(entry.typeId);
	const char *name = info ? info->name : "Unknown";

	ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 0.85f, 0.6f, 1.0f));
	ImGui::Text("%s", name);
	ImGui::PopStyleColor();
	ImGui::Separator();

	drawParamWidgets(entry, false, event);

	ImGui::PopID();
	ImGui::PopID();
	ed::EndNode();

	ed::PopStyleColor(2);
}

void StgNodeGraph::drawParamWidgets(StgScriptEntry &entry, bool isCondition,
				    StgEvent &event) {
	const ScriptEntryInfo *info = isCondition
					  ? findConditionInfo(entry.typeId)
					  : findActionInfo(entry.typeId);

	for (size_t i = 0; i < entry.params.size(); ++i) {
		ImGui::PushID(static_cast<int>(i));

		const char *paramHint = nullptr;
		if (info && i < 3 && info->paramNames[i][0]) {
			paramHint = info->paramNames[i];
		}

		char labelBuf[64];
		snprintf(labelBuf, sizeof(labelBuf), "%s",
			 paramHint ? paramHint : "Param");

		drawParamValue(labelBuf, entry.params[i], event, paramHint);
		ImGui::PopID();
	}
}

void StgNodeGraph::drawParamValue(const char *label, StgParamValue &param,
				  StgEvent &event, const char *paramHint) {
	ImGui::Text("%s:", label);
	ImGui::SameLine();
	ImGui::SetNextItemWidth(120.0f);

	switch (param.type) {
		case StgParamType::Int:
		case StgParamType::Enum: {
			if (isTroopIdHint(paramHint) && document_->stgData) {
				auto &units = document_->stgData->units();
				char preview[64];
				bool found = false;
				for (const auto &u : units) {
					if (static_cast<int32_t>(u.uniqueId) ==
					    param.intValue) {
						std::string dispName =
						    resolveDisplayName(
							u, *nameDict_);
						snprintf(
						    preview, sizeof(preview),
						    "%s (%d)", dispName.c_str(),
						    param.intValue);
						found = true;
						break;
					}
				}
				if (!found) {
					snprintf(preview, sizeof(preview),
						 "ID: %d", param.intValue);
				}

				// Deferred popup for troop combos inside nodes.
				char btnId[64];
				snprintf(btnId, sizeof(btnId), "%s##btn",
					 preview);
				if (ImGui::Button(btnId, ImVec2(120, 0))) {
					char popupId[64];
					snprintf(popupId, sizeof(popupId),
						 "troop_%p_%s",
						 static_cast<void *>(&param),
						 label);
					DeferredPopup popup;
					popup.id = popupId;
					popup.shouldOpen = true;
					popup.render = [this, &param, &event,
							&units]() {
						for (const auto &u : units) {
							std::string name =
							    resolveDisplayName(
								u, *nameDict_);
							char itemLabel[64];
							snprintf(
							    itemLabel,
							    sizeof(itemLabel),
							    "%s (%u)",
							    name.c_str(),
							    u.uniqueId);
							if (ImGui::MenuItem(
								itemLabel)) {
								param.intValue =
								    static_cast<
									int32_t>(
									u.uniqueId);
								event.modified =
								    true;
								document_
								    ->dirty =
								    true;
							}
						}
					};
					deferredPopups_.push_back(
					    std::move(popup));
				}
			} else {
				int val = param.intValue;
				if (ImGui::DragInt("##v", &val, 1, 0, 0)) {
					param.intValue = val;
					event.modified = true;
					document_->dirty = true;
				}
			}
			break;
		}
		case StgParamType::Float: {
			if (ImGui::DragFloat("##v", &param.floatValue, 0.1f,
					     0.0f, 0.0f, "%.3f")) {
				event.modified = true;
				document_->dirty = true;
			}
			break;
		}
		case StgParamType::String: {
			char strBuf[256];
			std::memset(strBuf, 0, sizeof(strBuf));
			std::strncpy(strBuf, param.stringValue.c_str(),
				     sizeof(strBuf) - 1);
			if (ImGui::InputText("##v", strBuf, sizeof(strBuf))) {
				param.stringValue = strBuf;
				event.modified = true;
				document_->dirty = true;
			}
			break;
		}
	}
}

void StgNodeGraph::handleContextMenus() {
	ed::NodeId contextNodeId;

	// Background context menu.
	if (ed::ShowBackgroundContextMenu()) {
		ImGui::OpenPopup("BackgroundMenu");
	}

	// Node context menu.
	if (ed::ShowNodeContextMenu(&contextNodeId)) {
		contextNodeId_ = contextNodeId;
		uint64_t rawId = contextNodeId_.Get();
		contextBlockIdx_ = static_cast<size_t>(decodeBlock(rawId));
		contextEventIdx_ = static_cast<size_t>(decodeEvent(rawId));
		NodeType type = decodeType(rawId);
		if (type == NodeType::EventGroup) {
			ImGui::OpenPopup("EventLabelMenu");
		} else {
			ImGui::OpenPopup("NodeMenu");
		}
	}

	ed::Suspend();

	if (ImGui::BeginPopup("BackgroundMenu")) {
		if (ImGui::MenuItem("Add Event")) {
			auto &blocks = document_->stgData->eventBlocks();
			if (blocks.empty()) {
				blocks.push_back({});
			}
			StgEvent newEvent;
			newEvent.description = "New Event";
			newEvent.modified = true;
			blocks[0].events.push_back(newEvent);
			document_->dirty = true;
		}
		ImGui::EndPopup();
	}

	if (ImGui::BeginPopup("EventLabelMenu")) {
		auto &blocks = document_->stgData->eventBlocks();
		if (contextBlockIdx_ < blocks.size() &&
		    contextEventIdx_ < blocks[contextBlockIdx_].events.size()) {
			auto &event =
			    blocks[contextBlockIdx_].events[contextEventIdx_];

			if (ImGui::MenuItem("Add Condition")) {
				event.conditions.push_back({});
				event.modified = true;
				document_->dirty = true;
			}
			if (ImGui::MenuItem("Add Action")) {
				event.actions.push_back({});
				event.modified = true;
				document_->dirty = true;
			}
			ImGui::Separator();
			if (ImGui::MenuItem("Delete Event")) {
				blocks[contextBlockIdx_].events.erase(
				    blocks[contextBlockIdx_].events.begin() +
				    static_cast<ptrdiff_t>(contextEventIdx_));
				document_->dirty = true;
			}
		}
		ImGui::EndPopup();
	}

	if (ImGui::BeginPopup("NodeMenu")) {
		uint64_t rawId = contextNodeId_.Get();
		NodeType type = decodeType(rawId);
		size_t blockIdx = static_cast<size_t>(decodeBlock(rawId));
		size_t eventIdx = static_cast<size_t>(decodeEvent(rawId));
		size_t entryIdx = static_cast<size_t>(decodeEntry(rawId));

		auto &blocks = document_->stgData->eventBlocks();

		if (ImGui::MenuItem("Delete")) {
			if (blockIdx < blocks.size() &&
			    eventIdx < blocks[blockIdx].events.size()) {
				auto &event = blocks[blockIdx].events[eventIdx];
				if (type == NodeType::Condition &&
				    entryIdx < event.conditions.size()) {
					event.conditions.erase(
					    event.conditions.begin() +
					    static_cast<ptrdiff_t>(entryIdx));
					event.modified = true;
					document_->dirty = true;
				} else if (type == NodeType::Action &&
					   entryIdx < event.actions.size()) {
					event.actions.erase(
					    event.actions.begin() +
					    static_cast<ptrdiff_t>(entryIdx));
					event.modified = true;
					document_->dirty = true;
				}
			}
		}
		ImGui::EndPopup();
	}

	ed::Resume();

	// Handle Delete key for selected nodes.
	if (ed::HasSelectionChanged()) {
		// Selection change handling if needed in the future.
	}
}

void StgNodeGraph::renderDeferredPopups() {
	for (auto &popup : deferredPopups_) {
		if (popup.shouldOpen) {
			ImGui::OpenPopup(popup.id.c_str());
			popup.shouldOpen = false;
		}
		if (ImGui::BeginPopup(popup.id.c_str())) {
			popup.render();
			ImGui::EndPopup();
		}
	}
}

void StgNodeGraph::autoLayout() {
	if (!document_ || !document_->stgData) return;

	ed::SetCurrentEditor(context_);

	auto &blocks = document_->stgData->eventBlocks();

	constexpr float kEventSpacingY = 250.0f;
	constexpr float kCondColumnX = 100.0f;
	constexpr float kActionColumnX = 500.0f;
	constexpr float kLabelOffsetY = -40.0f;
	constexpr float kNodeSpacingY = 140.0f;

	float globalY = 50.0f;

	for (size_t b = 0; b < blocks.size(); ++b) {
		auto &block = blocks[b];
		for (size_t e = 0; e < block.events.size(); ++e) {
			auto &event = block.events[e];

			// Position event label node.
			ed::SetNodePosition(
			    ed::NodeId(eventGroupNode(b, e)),
			    ImVec2(kCondColumnX, globalY + kLabelOffsetY));

			// Condition nodes in left column.
			for (size_t c = 0; c < event.conditions.size(); ++c) {
				ed::SetNodePosition(
				    ed::NodeId(conditionNode(b, e, c)),
				    ImVec2(kCondColumnX,
					   globalY + c * kNodeSpacingY));
			}

			// Action nodes in right column.
			for (size_t a = 0; a < event.actions.size(); ++a) {
				ed::SetNodePosition(
				    ed::NodeId(actionNode(b, e, a)),
				    ImVec2(kActionColumnX,
					   globalY + a * kNodeSpacingY));
			}

			size_t maxEntries = std::max(event.conditions.size(),
						     event.actions.size());
			if (maxEntries == 0) maxEntries = 1;
			globalY += maxEntries * kNodeSpacingY + kEventSpacingY;
		}
	}

	ed::NavigateToContent(0.0f);
}

} // namespace kuf
