#include "ui/tabs/stg_editor_tab.h"

#include <imgui.h>

#include <algorithm>
#include <cctype>
#include <cstring>

#include "ui/imgui_helpers.h"
#include "ui/stg_display_helpers.h"

namespace kuf {

namespace {

const char *ucdNames[] = {"Player", "Enemy", "Ally", "Neutral"};

const char *directionNames[] = {"East", "NorthEast", "North", "NorthWest",
				"West", "SouthWest", "South", "SouthEast"};

ImVec4 ucdColor(UCD ucd) {
	switch (ucd) {
		case UCD::Player:
			return ImVec4(0.2f, 0.8f, 0.2f, 1.0f);
		case UCD::Enemy:
			return ImVec4(0.9f, 0.2f, 0.2f, 1.0f);
		case UCD::Ally:
			return ImVec4(0.2f, 0.5f, 0.9f, 1.0f);
		case UCD::Neutral:
			return ImVec4(0.7f, 0.7f, 0.7f, 1.0f);
	}
	return ImVec4(1.0f, 1.0f, 1.0f, 1.0f);
}

const char *jobTypeName(uint8_t jobType, const NameDictionary &dict) {
	if (isCharInfoJobType(jobType)) {
		const char *name = dict.charInfoName(jobType);
		if (name) return name;
	}
	if (jobType <= kMaxStandardJobType) {
		const char *name = dict.troopInfoName(jobType);
		if (name) return name;
	}
	return dict.charInfoName(jobType);
}

void drawJobTypeCombo(const char *label, uint8_t &current,
		      const NameDictionary &dict) {
	const char *currentName = jobTypeName(current, dict);

	char preview[64];
	if (currentName) {
		snprintf(preview, sizeof(preview), "%s (%d)", currentName,
			 current);
	} else {
		snprintf(preview, sizeof(preview), "Model %d", current);
	}

	if (BeginComboCentered(label, preview)) {
		// Standard job type IDs (0-42).
		for (int i = 0; i <= kMaxStandardJobType; ++i) {
			char itemLabel[64];
			const char *name =
			    jobTypeName(static_cast<uint8_t>(i), dict);
			if (!name) {
				snprintf(itemLabel, sizeof(itemLabel), "Job %d",
					 i);
			} else {
				snprintf(itemLabel, sizeof(itemLabel),
					 "%s (%d)", name, i);
			}
			bool selected = (current == i);
			if (ImGui::Selectable(itemLabel, selected)) {
				current = static_cast<uint8_t>(i);
			}
			if (selected) ImGui::SetItemDefaultFocus();
		}

		ImGui::Separator();

		// CharInfo entries above standard range.
		for (int i = kMaxStandardJobType + 1; i < 256; ++i) {
			const char *charName =
			    dict.charInfoName(static_cast<uint8_t>(i));
			if (!charName) continue;
			char itemLabel[64];
			snprintf(itemLabel, sizeof(itemLabel), "%s (%d)",
				 charName, i);
			bool selected = (current == i);
			if (ImGui::Selectable(itemLabel, selected)) {
				current = static_cast<uint8_t>(i);
			}
			if (selected) ImGui::SetItemDefaultFocus();
		}
		ImGui::EndCombo();
	}
}

const char *paramTypeName(StgParamType type) {
	switch (type) {
		case StgParamType::Int:
			return "Int";
		case StgParamType::Float:
			return "Float";
		case StgParamType::String:
			return "String";
		case StgParamType::Enum:
			return "Enum";
	}
	return "Unknown";
}

} // namespace

StgEditorTab::StgEditorTab(std::shared_ptr<OpenDocument> doc)
    : EditorTab(std::move(doc)) {
	if (document_ && !document_->path.empty()) {
		std::string soxDir = findGameDirectory(document_->path);
		if (!soxDir.empty()) {
			nameDictionary_.load(soxDir);
		}
	}
	if (document_) {
		nodeGraph_.initialize(document_, &nameDictionary_);
	}
}

void StgEditorTab::selectUnit(size_t index) {
	if (document_ && document_->stgData &&
	    index < document_->stgData->unitCount()) {
		selectedUnit_ = static_cast<int>(index);
		currentSection_ = Section::Units;
	}
}

void StgEditorTab::drawContent() {
	if (!document_ || !document_->stgData) {
		ImGui::TextDisabled("No STG data loaded");
		return;
	}

	float totalHeight = ImGui::GetContentRegionAvail().y;

	// Sidebar.
	ImGui::BeginChild("StgSidebar", ImVec2(120, totalHeight),
			  ImGuiChildFlags_Borders);
	drawSidebar();
	ImGui::EndChild();

	ImGui::SameLine();

	if (currentSection_ == Section::Header) {
		ImGui::BeginChild("StgHeaderContent", ImVec2(0, totalHeight),
				  ImGuiChildFlags_Borders);
		drawHeaderSection();
		ImGui::EndChild();
	} else if (currentSection_ == Section::Areas) {
		if (!document_->stgData->tailParsed()) {
			ImGui::BeginChild("StgAreasUnparsed",
					  ImVec2(0, totalHeight),
					  ImGuiChildFlags_Borders);
			ImGui::TextDisabled(
			    "Area section could not be parsed. Raw data is "
			    "preserved for round-trip safety.");
			ImGui::EndChild();
		} else {
			ImGui::BeginChild("StgAreaList",
					  ImVec2(230, totalHeight),
					  ImGuiChildFlags_Borders);
			drawAreaList();
			ImGui::EndChild();

			ImGui::SameLine();

			ImGui::BeginChild("StgAreaDetails",
					  ImVec2(0, totalHeight),
					  ImGuiChildFlags_Borders);
			if (selectedArea_ >= 0 &&
			    selectedArea_ <
				static_cast<int>(
				    document_->stgData->areas().size())) {
				drawAreaDetails(selectedArea_);
			} else {
				ImGui::TextDisabled("Select an area to edit");
			}
			ImGui::EndChild();
		}
	} else if (currentSection_ == Section::Variables) {
		if (!document_->stgData->tailParsed()) {
			ImGui::BeginChild("StgVarsUnparsed",
					  ImVec2(0, totalHeight),
					  ImGuiChildFlags_Borders);
			ImGui::TextDisabled(
			    "Variable section could not be parsed. Raw data is "
			    "preserved for round-trip safety.");
			ImGui::EndChild();
		} else {
			ImGui::BeginChild("StgVarList",
					  ImVec2(230, totalHeight),
					  ImGuiChildFlags_Borders);
			drawVariableList();
			ImGui::EndChild();

			ImGui::SameLine();

			ImGui::BeginChild("StgVarDetails",
					  ImVec2(0, totalHeight),
					  ImGuiChildFlags_Borders);
			if (selectedVariable_ >= 0 &&
			    selectedVariable_ <
				static_cast<int>(
				    document_->stgData->variables().size())) {
				drawVariableDetails(selectedVariable_);
			} else {
				ImGui::TextDisabled(
				    "Select a variable to edit");
			}
			ImGui::EndChild();
		}
	} else if (currentSection_ == Section::Events) {
		if (!document_->stgData->tailParsed()) {
			ImGui::BeginChild("StgEventsUnparsed",
					  ImVec2(0, totalHeight),
					  ImGuiChildFlags_Borders);
			ImGui::TextDisabled(
			    "Event section could not be parsed. Raw data is "
			    "preserved for round-trip safety.");
			ImGui::EndChild();
		} else {
			nodeGraph_.draw();
		}
	} else {
		ImGui::BeginChild("StgUnitList", ImVec2(230, totalHeight),
				  ImGuiChildFlags_Borders);
		drawUnitList();
		ImGui::EndChild();

		ImGui::SameLine();

		ImGui::BeginChild("StgUnitDetails", ImVec2(0, totalHeight),
				  ImGuiChildFlags_Borders);
		if (selectedUnit_ >= 0 &&
		    selectedUnit_ <
			static_cast<int>(document_->stgData->unitCount())) {
			drawUnitDetails(selectedUnit_);
		} else {
			ImGui::TextDisabled("Select a unit to edit");
		}
		ImGui::EndChild();
	}
}

void StgEditorTab::drawSidebar() {
	ImGui::Text("Sections");
	ImGui::Separator();

	if (ImGui::Selectable("Header", currentSection_ == Section::Header)) {
		currentSection_ = Section::Header;
	}
	if (ImGui::Selectable("Units", currentSection_ == Section::Units)) {
		currentSection_ = Section::Units;
	}

	bool hasParsedTail = document_->stgData->tailParsed();

	{
		size_t areaCount =
		    hasParsedTail ? document_->stgData->areas().size() : 0;
		char areasLabel[32];
		if (hasParsedTail) {
			snprintf(areasLabel, sizeof(areasLabel), "Areas (%zu)",
				 areaCount);
		} else {
			snprintf(areasLabel, sizeof(areasLabel),
				 "Areas (unparsed)");
		}
		if (ImGui::Selectable(areasLabel,
				      currentSection_ == Section::Areas)) {
			currentSection_ = Section::Areas;
		}
	}

	{
		size_t varCount =
		    hasParsedTail ? document_->stgData->variables().size() : 0;
		char varsLabel[32];
		if (hasParsedTail) {
			snprintf(varsLabel, sizeof(varsLabel),
				 "Variables (%zu)", varCount);
		} else {
			snprintf(varsLabel, sizeof(varsLabel),
				 "Variables (unparsed)");
		}
		if (ImGui::Selectable(varsLabel,
				      currentSection_ == Section::Variables)) {
			currentSection_ = Section::Variables;
		}
	}

	{
		const char *eventsLabel =
		    hasParsedTail ? "Events" : "Events (unparsed)";
		if (ImGui::Selectable(eventsLabel,
				      currentSection_ == Section::Events)) {
			currentSection_ = Section::Events;
		}
	}
}

void StgEditorTab::drawHeaderSection() {
	auto &hdr = document_->stgData->header();

	ImGui::Text("Mission Header");
	ImGui::Separator();

	ImGui::Text("Format Magic: 0x%X", hdr.formatMagic);

	char buf[64];

	auto stringInput = [&](const char *label, std::string &str) {
		std::memset(buf, 0, sizeof(buf));
		std::strncpy(buf, str.c_str(), sizeof(buf) - 1);
		if (InputTextCentered(label, buf, sizeof(buf))) {
			str = buf;
			document_->dirty = true;
		}
	};

	if (ImGui::CollapsingHeader("File References",
				    ImGuiTreeNodeFlags_DefaultOpen)) {
		stringInput("Map File", hdr.mapFile);
		stringInput("Bitmap File", hdr.bitmapFile);
		stringInput("Default Camera", hdr.defaultCameraFile);
		stringInput("User Camera", hdr.userCameraFile);
		stringInput("Settings File", hdr.settingsFile);
		stringInput("Sky/Cloud Effects", hdr.skyCloudEffects);
		stringInput("AI Script", hdr.aiScriptFile);
		stringInput("Cubemap Texture", hdr.cubemapTexture);
	}

	ImGui::Separator();
	ImGui::Text("Unit Count: %u", hdr.unitCount);
}

void StgEditorTab::drawUnitList() {
	const auto &units = document_->stgData->units();

	for (size_t i = 0; i < units.size(); ++i) {
		const auto &unit = units[i];
		bool selected = (selectedUnit_ == static_cast<int>(i));

		// Color-code by UCD, dimming disabled units.
		ImVec4 color = ucdColor(unit.ucd);
		if (!unit.isEnabled) {
			color.w = 0.4f;
		}
		ImGui::PushStyleColor(ImGuiCol_Text, color);

		std::string displayName =
		    resolveDisplayName(unit, nameDictionary_);

		char label[64];
		snprintf(label, sizeof(label), "[%zu] %s", i,
			 displayName.c_str());

		if (ImGui::Selectable(label, selected)) {
			selectedUnit_ = static_cast<int>(i);
		}

		ImGui::PopStyleColor();

		if (ImGui::IsItemHovered()) {
			ImGui::SetTooltip(
			    "ID: %u | %s | TroopIdx: %d | Job: %d | Lv%d%s",
			    unit.uniqueId, ucdNames[static_cast<int>(unit.ucd)],
			    unit.troopInfoIndex, unit.leaderJobType,
			    unit.leaderLevel,
			    unit.isEnabled ? "" : " [Disabled]");
		}
	}
}

void StgEditorTab::drawUnitDetails(size_t index) {
	auto &unit = document_->stgData->units()[index];

	std::string detailDisplayName =
	    resolveDisplayName(unit, nameDictionary_);
	ImGui::Text("[%zu] %s", index, detailDisplayName.c_str());
	ImGui::Separator();

	if (ImGui::CollapsingHeader("Core", ImGuiTreeNodeFlags_DefaultOpen)) {
		ImGui::Text("Display Name: %s", detailDisplayName.c_str());

		if (ImGui::TreeNode("Advanced##name")) {
			char nameBuf[32];
			std::memset(nameBuf, 0, sizeof(nameBuf));
			std::strncpy(nameBuf, unit.unitName.c_str(),
				     sizeof(nameBuf) - 1);
			if (InputTextCentered("Internal Name", nameBuf,
					      sizeof(nameBuf))) {
				unit.unitName = nameBuf;
				document_->dirty = true;
			}
			if (ImGui::IsItemHovered())
				ImGui::SetTooltip("File-internal CP949 name "
						  "(Korean). Changing this "
						  "may break save references.");
			ImGui::TreePop();
		}

		int uid = static_cast<int>(unit.uniqueId);
		if (ImGui::DragInt("Unique ID", &uid, 1, 0, 0)) {
			unit.uniqueId = static_cast<uint32_t>(std::max(0, uid));
			document_->dirty = true;
		}

		int ucdIdx = static_cast<int>(unit.ucd);
		if (ComboCentered("UCD", &ucdIdx, ucdNames,
				  IM_ARRAYSIZE(ucdNames))) {
			unit.ucd = static_cast<UCD>(ucdIdx);
			document_->dirty = true;
		}

		bool hero = unit.isHero != 0;
		if (ImGui::Checkbox("Is Hero", &hero)) {
			unit.isHero = hero ? 1 : 0;
			document_->dirty = true;
		}

		ImGui::SameLine();
		bool enabled = unit.isEnabled != 0;
		if (ImGui::Checkbox("Is Enabled", &enabled)) {
			unit.isEnabled = enabled ? 1 : 0;
			document_->dirty = true;
		}

		if (ImGui::DragFloat("Leader HP Override",
				     &unit.leaderHpOverride, 1.0f, -1.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::IsItemHovered())
			ImGui::SetTooltip("-1.0 = use default");

		if (ImGui::DragFloat("Unit HP Override", &unit.unitHpOverride,
				     1.0f, -1.0f, 100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::IsItemHovered())
			ImGui::SetTooltip("-1.0 = use default");
	}

	if (ImGui::CollapsingHeader("Position",
				    ImGuiTreeNodeFlags_DefaultOpen)) {
		if (ImGui::DragFloat("X", &unit.positionX, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::DragFloat("Y", &unit.positionY, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}

		int dirIdx = static_cast<int>(unit.direction);
		if (ComboCentered("Direction", &dirIdx, directionNames,
				  IM_ARRAYSIZE(directionNames))) {
			unit.direction = static_cast<Direction>(dirIdx);
			document_->dirty = true;
		}
	}

	if (ImGui::CollapsingHeader("Leader", ImGuiTreeNodeFlags_DefaultOpen)) {
		drawJobTypeCombo("Job Type", unit.leaderJobType,
				 nameDictionary_);

		int modelId = unit.leaderModelId;
		if (ImGui::DragInt("Model ID", &modelId, 1, 0, 255)) {
			unit.leaderModelId =
			    static_cast<uint8_t>(std::clamp(modelId, 0, 255));
			document_->dirty = true;
		}

		int wmId = unit.leaderWorldmapId;
		if (ImGui::DragInt("Worldmap ID", &wmId, 1, 0, 255)) {
			unit.leaderWorldmapId =
			    static_cast<uint8_t>(std::clamp(wmId, 0, 255));
			document_->dirty = true;
		}
		if (ImGui::IsItemHovered())
			ImGui::SetTooltip(
			    "0xFF = standalone (no campaign save). Other "
			    "values "
			    "link to barracks slot - DO NOT reuse.");

		int level = unit.leaderLevel;
		if (ImGui::DragInt("Level", &level, 1, 1, 99)) {
			unit.leaderLevel =
			    static_cast<uint8_t>(std::clamp(level, 1, 99));
			document_->dirty = true;
		}
	}

	if (ImGui::CollapsingHeader("Skills")) {
		for (int i = 0; i < 4; ++i) {
			ImGui::PushID(i);
			char skillLabel[32];
			snprintf(skillLabel, sizeof(skillLabel), "Skill %d",
				 i + 1);

			if (unit.leaderSkills[i].skillId == 255) {
				ImGui::TextDisabled("%s: Empty", skillLabel);
				ImGui::SameLine();
				if (ImGui::SmallButton("Set")) {
					unit.leaderSkills[i].skillId = 0;
					unit.leaderSkills[i].level = 1;
					document_->dirty = true;
				}
			} else {
				int skillId = unit.leaderSkills[i].skillId;
				int skillLv = unit.leaderSkills[i].level;

				ImGui::Text("%s:", skillLabel);
				ImGui::SameLine();
				ImGui::SetNextItemWidth(120);
				if (ImGui::DragInt("##id", &skillId, 1, 0,
						   254)) {
					unit.leaderSkills[i].skillId =
					    static_cast<uint8_t>(
						std::clamp(skillId, 0, 254));
					document_->dirty = true;
				}
				ImGui::SameLine();
				ImGui::Text("Lv:");
				ImGui::SameLine();
				ImGui::SetNextItemWidth(80);
				if (ImGui::DragInt("##lv", &skillLv, 1, 0,
						   255)) {
					unit.leaderSkills[i].level =
					    static_cast<uint8_t>(
						std::clamp(skillLv, 0, 255));
					document_->dirty = true;
				}
				ImGui::SameLine();
				if (ImGui::SmallButton("Clear")) {
					unit.leaderSkills[i].skillId = 255;
					unit.leaderSkills[i].level = 255;
					document_->dirty = true;
				}
			}
			ImGui::PopID();
		}
	}

	if (ImGui::CollapsingHeader("Abilities")) {
		for (int i = 0; i < 23; ++i) {
			ImGui::PushID(i + 100);
			char abilLabel[32];
			snprintf(abilLabel, sizeof(abilLabel), "Slot %d", i);

			int val = unit.leaderAbilities[i];
			if (val == -1) {
				ImGui::TextDisabled("%s: Empty", abilLabel);
				ImGui::SameLine();
				if (ImGui::SmallButton("Set")) {
					unit.leaderAbilities[i] = 0;
					document_->dirty = true;
				}
			} else {
				ImGui::SetNextItemWidth(120);
				if (ImGui::DragInt(abilLabel, &val)) {
					unit.leaderAbilities[i] = val;
					document_->dirty = true;
				}
				ImGui::SameLine();
				if (ImGui::SmallButton("Clear")) {
					unit.leaderAbilities[i] = -1;
					document_->dirty = true;
				}
			}
			ImGui::PopID();
		}
	}

	if (ImGui::CollapsingHeader("Officers")) {
		int count = static_cast<int>(unit.officerCount);
		if (ImGui::SliderInt("Officer Count", &count, 0, 2)) {
			unit.officerCount = static_cast<uint32_t>(count);
			document_->dirty = true;
		}

		if (unit.officerCount >= 1) {
			drawOfficerSection("Officer 1", unit.officer1, true);
		}
		if (unit.officerCount >= 2) {
			drawOfficerSection("Officer 2", unit.officer2, true);
		}
	}

	if (ImGui::CollapsingHeader("Unit Configuration")) {
		int troopIdx = unit.troopInfoIndex;
		if (ImGui::DragInt("TroopInfo Index", &troopIdx, 1, 0, 0)) {
			unit.troopInfoIndex = troopIdx;
			document_->dirty = true;
		}
		if (ImGui::IsItemHovered())
			ImGui::SetTooltip(
			    "References TroopInfo.sox. Negative values are "
			    "computed from formation type at runtime.");

		int formation = static_cast<int>(unit.formationType);
		if (ImGui::DragInt("Formation", &formation, 1, 0, 0)) {
			unit.formationType =
			    static_cast<uint32_t>(std::max(0, formation));
			document_->dirty = true;
		}

		int animConfig = static_cast<int>(unit.unitAnimConfig);
		if (ImGui::DragInt("Anim/Grid Config", &animConfig, 1, 0, 0)) {
			unit.unitAnimConfig =
			    static_cast<uint32_t>(std::max(0, animConfig));
			document_->dirty = true;
		}

		int gx = static_cast<int>(unit.gridX);
		int gy = static_cast<int>(unit.gridY);
		if (ImGui::DragInt("Grid X", &gx, 1, 1, 0)) {
			unit.gridX = static_cast<uint32_t>(std::max(1, gx));
			document_->dirty = true;
		}
		if (ImGui::DragInt("Grid Y", &gy, 1, 1, 0)) {
			unit.gridY = static_cast<uint32_t>(std::max(1, gy));
			document_->dirty = true;
		}
		ImGui::Text("Total Units: %u",
			    static_cast<uint32_t>(unit.gridX) * unit.gridY);
	}

	if (ImGui::CollapsingHeader("Stat Overrides")) {
		ImGui::TextDisabled("Values of -1.0 use TroopInfo defaults");
		ImGui::Separator();

		for (int i = 0; i < 22; ++i) {
			ImGui::PushID(i + 200);
			char label[32];
			snprintf(label, sizeof(label), "Override %d", i);

			if (ImGui::DragFloat(label, &unit.statOverrides[i],
					     1.0f, -1.0f, 100000.0f, "%.1f")) {
				document_->dirty = true;
			}
			ImGui::PopID();
		}
	}
}

void StgEditorTab::drawOfficerSection(const char *label, OfficerData &officer,
				      bool active) {
	if (!active) return;

	ImGui::PushID(label);
	if (ImGui::TreeNode(label)) {
		drawJobTypeCombo("Job Type", officer.jobType, nameDictionary_);

		int modelId = officer.modelId;
		if (ImGui::DragInt("Model ID", &modelId, 1, 0, 255)) {
			officer.modelId =
			    static_cast<uint8_t>(std::clamp(modelId, 0, 255));
			document_->dirty = true;
		}

		int wmId = officer.worldmapId;
		if (ImGui::DragInt("Worldmap ID", &wmId, 1, 0, 255)) {
			officer.worldmapId =
			    static_cast<uint8_t>(std::clamp(wmId, 0, 255));
			document_->dirty = true;
		}

		int level = officer.level;
		if (ImGui::DragInt("Level", &level, 1, 1, 99)) {
			officer.level =
			    static_cast<uint8_t>(std::clamp(level, 1, 99));
			document_->dirty = true;
		}

		if (ImGui::TreeNode("Abilities")) {
			ImGui::TextDisabled(
			    "Officers store skills/passives here (IDs). Magic "
			    "skill lv5+ unlocks actives.");
			for (int i = 0; i < 23; ++i) {
				ImGui::PushID(i + 300);
				int val = officer.abilities[i];
				if (val == -1) {
					ImGui::TextDisabled("Slot %d: Empty",
							    i);
					ImGui::SameLine();
					if (ImGui::SmallButton("Set")) {
						officer.abilities[i] = 0;
						document_->dirty = true;
					}
				} else {
					ImGui::SetNextItemWidth(120);
					if (ImGui::DragInt(
						("Slot " + std::to_string(i))
						    .c_str(),
						&val)) {
						officer.abilities[i] = val;
						document_->dirty = true;
					}
					ImGui::SameLine();
					if (ImGui::SmallButton("Clear")) {
						officer.abilities[i] = -1;
						document_->dirty = true;
					}
				}
				ImGui::PopID();
			}
			ImGui::TreePop();
		}

		ImGui::TreePop();
	}
	ImGui::PopID();
}

void StgEditorTab::drawAreaList() {
	auto &areas = document_->stgData->areas();

	for (size_t i = 0; i < areas.size(); ++i) {
		const auto &area = areas[i];
		bool selected = (selectedArea_ == static_cast<int>(i));

		std::string displayAreaDesc =
		    nameDictionary_.translate(area.description);
		if (displayAreaDesc.empty()) displayAreaDesc = area.description;

		char label[96];
		if (displayAreaDesc.empty()) {
			snprintf(label, sizeof(label), "[%zu] Area %u", i,
				 area.areaId);
		} else {
			snprintf(label, sizeof(label), "[%zu] %s (ID %u)", i,
				 displayAreaDesc.c_str(), area.areaId);
		}

		if (ImGui::Selectable(label, selected)) {
			selectedArea_ = static_cast<int>(i);
		}

		if (ImGui::IsItemHovered()) {
			ImGui::SetTooltip("Bounds: (%.0f, %.0f) - (%.0f, %.0f)",
					  area.boundX1, area.boundY1,
					  area.boundX2, area.boundY2);
		}
	}
}

void StgEditorTab::drawAreaDetails(size_t index) {
	auto &area = document_->stgData->areas()[index];

	ImGui::Text("Area %zu", index);
	ImGui::Separator();

	std::string translatedAreaDesc =
	    nameDictionary_.translate(area.description);
	const std::string &displayDesc =
	    translatedAreaDesc.empty() ? area.description : translatedAreaDesc;

	char descBuf[32];
	std::memset(descBuf, 0, sizeof(descBuf));
	std::strncpy(descBuf, displayDesc.c_str(), sizeof(descBuf) - 1);
	if (InputTextCentered("Description", descBuf, sizeof(descBuf))) {
		std::string input(descBuf);
		std::string korean = nameDictionary_.reverseTranslate(input);
		area.description = korean.empty() ? input : korean;
		document_->dirty = true;
	}

	int areaId = static_cast<int>(area.areaId);
	if (ImGui::DragInt("Area ID", &areaId, 1, 0, 0)) {
		area.areaId = static_cast<uint32_t>(std::max(0, areaId));
		document_->dirty = true;
	}

	if (ImGui::CollapsingHeader("Bounds", ImGuiTreeNodeFlags_DefaultOpen)) {
		if (ImGui::DragFloat("X1", &area.boundX1, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::DragFloat("Y1", &area.boundY1, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::DragFloat("X2", &area.boundX2, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}
		if (ImGui::DragFloat("Y2", &area.boundY2, 10.0f, -100000.0f,
				     100000.0f, "%.1f")) {
			document_->dirty = true;
		}

		float w = std::abs(area.boundX2 - area.boundX1);
		float h = std::abs(area.boundY2 - area.boundY1);
		ImGui::Text("Size: %.0f x %.0f", w, h);
	}
}

void StgEditorTab::drawVariableList() {
	auto &vars = document_->stgData->variables();

	for (size_t i = 0; i < vars.size(); ++i) {
		const auto &var = vars[i];
		bool selected = (selectedVariable_ == static_cast<int>(i));

		std::string displayVarName =
		    nameDictionary_.translate(var.name);
		if (displayVarName.empty()) displayVarName = var.name;

		char label[96];
		snprintf(label, sizeof(label), "[%u] %s", var.variableId,
			 displayVarName.c_str());

		if (ImGui::Selectable(label, selected)) {
			selectedVariable_ = static_cast<int>(i);
		}

		if (ImGui::IsItemHovered()) {
			const char *typeName =
			    paramTypeName(var.initialValue.type);
			if (var.initialValue.type == StgParamType::String) {
				ImGui::SetTooltip(
				    "Type: %s | Initial: \"%s\"", typeName,
				    var.initialValue.stringValue.c_str());
			} else if (var.initialValue.type ==
				   StgParamType::Float) {
				ImGui::SetTooltip("Type: %s | Initial: %.3f",
						  typeName,
						  var.initialValue.floatValue);
			} else {
				ImGui::SetTooltip("Type: %s | Initial: %d",
						  typeName,
						  var.initialValue.intValue);
			}
		}
	}
}

void StgEditorTab::drawVariableDetails(size_t index) {
	auto &var = document_->stgData->variables()[index];

	ImGui::Text("Variable %zu", index);
	ImGui::Separator();

	std::string translatedVarName = nameDictionary_.translate(var.name);
	const std::string &displayName =
	    translatedVarName.empty() ? var.name : translatedVarName;

	char nameBuf[64];
	std::memset(nameBuf, 0, sizeof(nameBuf));
	std::strncpy(nameBuf, displayName.c_str(), sizeof(nameBuf) - 1);
	if (InputTextCentered("Name", nameBuf, sizeof(nameBuf))) {
		std::string input(nameBuf);
		std::string korean = nameDictionary_.reverseTranslate(input);
		var.name = korean.empty() ? input : korean;
		document_->dirty = true;
	}

	int varId = static_cast<int>(var.variableId);
	if (ImGui::DragInt("Variable ID", &varId, 1, 0, 0)) {
		var.variableId = static_cast<uint32_t>(std::max(0, varId));
		document_->dirty = true;
	}

	ImGui::Separator();
	ImGui::Text("Initial Value");

	// Type selector.
	int typeIdx = static_cast<int>(var.initialValue.type);
	const char *typeNames[] = {"Int", "Float", "String", "Enum"};
	if (ComboCentered("Type", &typeIdx, typeNames,
			  IM_ARRAYSIZE(typeNames))) {
		var.initialValue.type = static_cast<StgParamType>(typeIdx);
		document_->dirty = true;
	}

	// Value editor based on type.
	switch (var.initialValue.type) {
		case StgParamType::Int:
		case StgParamType::Enum: {
			int val = var.initialValue.intValue;
			if (ImGui::DragInt("Value", &val, 1, 0, 0)) {
				var.initialValue.intValue = val;
				document_->dirty = true;
			}
			break;
		}
		case StgParamType::Float: {
			if (ImGui::DragFloat("Value",
					     &var.initialValue.floatValue, 0.1f,
					     0.0f, 0.0f, "%.3f")) {
				document_->dirty = true;
			}
			break;
		}
		case StgParamType::String: {
			char strBuf[256];
			std::memset(strBuf, 0, sizeof(strBuf));
			std::strncpy(strBuf,
				     var.initialValue.stringValue.c_str(),
				     sizeof(strBuf) - 1);
			if (InputTextCentered("Value", strBuf,
					      sizeof(strBuf))) {
				var.initialValue.stringValue = strBuf;
				document_->dirty = true;
			}
			break;
		}
	}
}

} // namespace kuf
