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

static void drawPinIcon() {
	ImDrawList *dl = ImGui::GetWindowDrawList();
	ImVec2 pos = ImGui::GetCursorScreenPos();
	float lineH = ImGui::GetTextLineHeight();
	float r = 5.0f;
	ImVec2 center(pos.x + r, pos.y + lineH * 0.5f);
	dl->AddCircleFilled(center, r, IM_COL32(200, 200, 200, 255));
	ImGui::Dummy(ImVec2(r * 2.0f, lineH));
}

static std::string translateName(const NameDictionary *dict,
				 const std::string &raw) {
	std::string t = dict->translate(raw);
	return t.empty() ? raw : t;
}

static void drawChangeTypeMenu(StgScriptEntry &entry, StgEvent &event,
			       const ScriptEntryInfo *catalog, size_t count,
			       bool &dirty) {
	if (ImGui::BeginMenu("Change Type")) {
		for (size_t i = 0; i < count; ++i) {
			bool selected = entry.typeId == catalog[i].id;
			if (ImGui::MenuItem(catalog[i].name, nullptr,
					    selected)) {
				entry.typeId = catalog[i].id;
				entry.params.resize(catalog[i].paramCount);
				event.modified = true;
				dirty = true;
			}
		}
		ImGui::EndMenu();
	}
}

static ImColor eventColor(size_t eventIdx) {
	constexpr ImColor palette[] = {
	    ImColor(100, 160, 255, 200), ImColor(255, 160, 100, 200),
	    ImColor(100, 255, 160, 200), ImColor(255, 100, 160, 200),
	    ImColor(160, 100, 255, 200), ImColor(255, 255, 100, 200),
	    ImColor(100, 255, 255, 200), ImColor(255, 100, 255, 200),
	};
	return palette[eventIdx % 8];
}

void StgNodeGraph::openIdPopup(const char *label, StgParamValue &param,
			       const char *preview, const char *popupPrefix,
			       std::function<void()> renderFn) {
	char btnId[128];
	snprintf(btnId, sizeof(btnId), "%s##btn", preview);
	if (ImGui::Button(btnId, ImVec2(120, 0))) {
		char popupId[64];
		snprintf(popupId, sizeof(popupId), "%s_%p_%s", popupPrefix,
			 static_cast<void *>(&param), label);
		DeferredPopup popup;
		popup.id = popupId;
		popup.shouldOpen = true;
		popup.render = std::move(renderFn);
		deferredPopups_.push_back(std::move(popup));
	}
}

template <typename Container, typename IdFn, typename NameFn>
void StgNodeGraph::drawIdDropdown(const char *label, StgParamValue &param,
				  StgEvent &event, const char *popupPrefix,
				  const Container &items, IdFn idFn,
				  NameFn nameFn) {
	char preview[128];
	bool found = false;
	for (const auto &item : items) {
		if (static_cast<int32_t>(idFn(item)) == param.intValue) {
			snprintf(preview, sizeof(preview), "%s (%d)",
				 nameFn(item).c_str(), param.intValue);
			found = true;
			break;
		}
	}
	if (!found) {
		snprintf(preview, sizeof(preview), "ID: %d", param.intValue);
	}

	openIdPopup(
	    label, param, preview, popupPrefix,
	    [this, &param, &event, &items, idFn, nameFn]() {
		    for (const auto &item : items) {
			    std::string name = nameFn(item);
			    char il[128];
			    snprintf(il, sizeof(il), "%s (%u)", name.c_str(),
				     static_cast<uint32_t>(idFn(item)));
			    if (ImGui::MenuItem(il)) {
				    param.intValue =
					static_cast<int32_t>(idFn(item));
				    event.modified = true;
				    document_->dirty = true;
			    }
		    }
	    });
}

void StgNodeGraph::drawEventIdParam(const char *label, StgParamValue &param,
				    StgEvent &event) {
	auto &blocks = document_->stgData->eventBlocks();

	char preview[128];
	bool found = false;
	for (const auto &b : blocks) {
		for (const auto &ev : b.events) {
			if (static_cast<int32_t>(ev.eventId) ==
			    param.intValue) {
				snprintf(
				    preview, sizeof(preview), "%s (%d)",
				    translateName(nameDict_, ev.description)
					.c_str(),
				    param.intValue);
				found = true;
				break;
			}
		}
		if (found) break;
	}
	if (!found) {
		snprintf(preview, sizeof(preview), "ID: %d", param.intValue);
	}

	openIdPopup(label, param, preview, "evt",
		    [this, &param, &event, &blocks]() {
			    for (const auto &b : blocks) {
				    for (const auto &ev : b.events) {
					    auto name = translateName(
						nameDict_, ev.description);
					    char il[128];
					    snprintf(il, sizeof(il), "%s (%u)",
						     name.c_str(), ev.eventId);
					    if (ImGui::MenuItem(il)) {
						    param.intValue =
							static_cast<int32_t>(
							    ev.eventId);
						    event.modified = true;
						    document_->dirty = true;
					    }
				    }
			    }
		    });
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

	// Handle drag-to-create.
	if (ed::BeginCreate()) {
		ed::PinId pinId;
		if (ed::QueryNewNode(&pinId)) {
			if (ed::AcceptNewItem()) {
				uint64_t rawPin = pinId.Get();
				createNodePending_ = true;
				createBlockIdx_ =
				    static_cast<size_t>(decodeBlock(rawPin));
				createEventIdx_ =
				    static_cast<size_t>(decodeEvent(rawPin));
				createIsAction_ = (decodeSub(rawPin) == 0);
			}
		}

		ed::PinId startId, endId;
		if (ed::QueryNewLink(&startId, &endId)) {
			// Links are implicit (all conditions <-> all actions
			// per event), so reject all direct pin-to-pin drags.
			ed::RejectNewItem();
		}
		ed::EndCreate();
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

	ImGui::BeginGroup();

	ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(0.7f, 0.85f, 1.0f, 1.0f));
	ImGui::Text("%s", name);
	ImGui::PopStyleColor();

	ImVec2 separatorPos = ImGui::GetCursorScreenPos();
	ImGui::Dummy(ImVec2(0, 1));

	// Two-column layout: params left, output pin right.
	ImGui::BeginGroup();
	drawParamWidgets(entry, true, event);
	if (entry.params.empty()) ImGui::Dummy(ImVec2(120.0f, 0));
	ImGui::EndGroup();

	ImGui::SameLine(0, 16.0f);

	ImGui::BeginGroup();
	ed::BeginPin(
	    ed::PinId(conditionOutputPin(blockIdx, eventIdx, entryIdx)),
	    ed::PinKind::Output);
	drawPinIcon();
	ed::EndPin();
	ImGui::EndGroup();

	ImGui::EndGroup();

	float contentWidth = ImGui::GetItemRectSize().x;
	ImGui::GetWindowDrawList()->AddLine(
	    separatorPos, ImVec2(separatorPos.x + contentWidth, separatorPos.y),
	    IM_COL32(60, 100, 160, 230), 1.0f);

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

	const ScriptEntryInfo *info = findActionInfo(entry.typeId);
	const char *name = info ? info->name : "Unknown";

	ImGui::BeginGroup();

	ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 0.85f, 0.6f, 1.0f));
	ImGui::Text("%s", name);
	ImGui::PopStyleColor();

	ImVec2 separatorPos = ImGui::GetCursorScreenPos();
	ImGui::Dummy(ImVec2(0, 1));

	// Two-column layout: input pin left, params right.
	ImGui::BeginGroup();
	ed::BeginPin(ed::PinId(actionInputPin(blockIdx, eventIdx, entryIdx)),
		     ed::PinKind::Input);
	drawPinIcon();
	ed::EndPin();
	ImGui::EndGroup();

	ImGui::SameLine(0, 8.0f);

	ImGui::BeginGroup();
	drawParamWidgets(entry, false, event);
	if (entry.params.empty()) ImGui::Dummy(ImVec2(120.0f, 0));
	ImGui::EndGroup();

	ImGui::EndGroup();

	float contentWidth = ImGui::GetItemRectSize().x;
	ImGui::GetWindowDrawList()->AddLine(
	    separatorPos, ImVec2(separatorPos.x + contentWidth, separatorPos.y),
	    IM_COL32(160, 100, 40, 230), 1.0f);

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
				drawIdDropdown(
				    label, param, event, "troop", units,
				    [](const StgUnit &u) { return u.uniqueId; },
				    [this](const StgUnit &u) {
					    return resolveDisplayName(
						u, *nameDict_);
				    });
			} else if (isAreaIdHint(paramHint) &&
				   document_->stgData) {
				drawIdDropdown(
				    label, param, event, "area",
				    document_->stgData->areas(),
				    [](const StgArea &a) { return a.areaId; },
				    [this](const StgArea &a) {
					    return translateName(nameDict_,
								 a.description);
				    });
			} else if (isVariableIdHint(paramHint) &&
				   document_->stgData) {
				drawIdDropdown(
				    label, param, event, "var",
				    document_->stgData->variables(),
				    [](const StgVariable &v) {
					    return v.variableId;
				    },
				    [this](const StgVariable &v) {
					    return translateName(nameDict_,
								 v.name);
				    });
			} else if (isEventIdHint(paramHint) &&
				   document_->stgData) {
				drawEventIdParam(label, param, event);
			} else if (isTriggerIdHint(paramHint) &&
				   document_->stgData) {
				drawEventIdParam(label, param, event);
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

	if (createNodePending_) {
		ImGui::OpenPopup("CreateNodeMenu");
		createNodePending_ = false;
	}

	ed::Suspend();

	if (ImGui::BeginPopup("CreateNodeMenu")) {
		auto &blocks = document_->stgData->eventBlocks();
		if (createBlockIdx_ < blocks.size() &&
		    createEventIdx_ < blocks[createBlockIdx_].events.size()) {
			auto &ev =
			    blocks[createBlockIdx_].events[createEventIdx_];
			auto addItem = [&](const char *lbl,
					   std::vector<StgScriptEntry> &v) {
				if (ImGui::MenuItem(lbl)) {
					v.push_back({});
					ev.modified = true;
					document_->dirty = true;
				}
			};
			if (createIsAction_) {
				addItem("Add Action", ev.actions);
				addItem("Add Condition", ev.conditions);
			} else {
				addItem("Add Condition", ev.conditions);
				addItem("Add Action", ev.actions);
			}
		}
		ImGui::EndPopup();
	}

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

		if (blockIdx < blocks.size() &&
		    eventIdx < blocks[blockIdx].events.size()) {
			auto &event = blocks[blockIdx].events[eventIdx];

			if (type == NodeType::Condition &&
			    entryIdx < event.conditions.size()) {
				drawChangeTypeMenu(event.conditions[entryIdx],
						   event, kConditions,
						   kConditionCount,
						   document_->dirty);
			} else if (type == NodeType::Action &&
				   entryIdx < event.actions.size()) {
				drawChangeTypeMenu(
				    event.actions[entryIdx], event, kActions,
				    kActionCount, document_->dirty);
			}

			ImGui::Separator();

			if (ImGui::MenuItem("Delete")) {
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
