#include "formats/stg_format.h"

#include "core/text_encoding.h"
#include "parsers/kuf_stg.h"

#include <algorithm>
#include <cstring>
#include <string_view>

namespace kuf {

namespace {

template <size_t N>
std::array<uint8_t, N> copyFixedBytes(const uint8_t (&bytes)[N]) {
	std::array<uint8_t, N> copy{};
	std::copy_n(bytes, N, copy.begin());
	return copy;
}

template <size_t N> std::string visibleFixedBytes(const uint8_t (&bytes)[N]) {
	const auto *end = std::find(bytes, bytes + N, uint8_t{0});
	return {reinterpret_cast<const char *>(bytes),
		reinterpret_cast<const char *>(end)};
}

template <size_t N>
STGFixedTextImage<N> UTF8TextImage(const uint8_t (&bytes)[N]) {
	return {copyFixedBytes(bytes), visibleFixedBytes(bytes)};
}

template <size_t N>
STGFixedTextImage<N> CP949TextImage(const uint8_t (&bytes)[N]) {
	auto source = visibleFixedBytes(bytes);
	return {copyFixedBytes(bytes), cp949ToUtf8(source)};
}

STGSaveError textError(STGSaveErrorCode code, std::string field,
		       std::string_view detail) {
	auto message = "Cannot save " + field + ": " + std::string(detail);
	return {code, std::move(field), std::move(message)};
}

template <size_t N>
bool writeFixedText(const std::string &current,
		    const STGFixedTextImage<N> &image, uint8_t (&output)[N],
		    bool CP949, std::string field,
		    std::optional<STGSaveError> &error) {
	if (current == image.decoded) {
		std::copy(image.bytes.begin(), image.bytes.end(), output);
		return true;
	}

	if (current.find('\0') != std::string::npos) {
		error =
		    textError(STGSaveErrorCode::EmbeddedZero, std::move(field),
			      "text contains an embedded zero byte");
		return false;
	}

	std::string encoded;
	if (CP949) {
		if (!isValidUTF8(current)) {
			error = textError(STGSaveErrorCode::InvalidUTF8,
					  std::move(field),
					  "text is not valid UTF8");
			return false;
		}
		auto converted = UTF8ToCP949Checked(current);
		if (!converted) {
			error = textError(
			    STGSaveErrorCode::Unrepresentable, std::move(field),
			    "text cannot be represented in CP949");
			return false;
		}
		encoded = std::move(*converted);
	} else {
		if (!isValidUTF8(current)) {
			error = textError(STGSaveErrorCode::InvalidUTF8,
					  std::move(field),
					  "text is not valid UTF8");
			return false;
		}
		encoded = current;
	}

	if (encoded.size() >= N) {
		error =
		    textError(STGSaveErrorCode::TextTooLong, std::move(field),
			      "encoded text exceeds the fixed field");
		return false;
	}

	std::fill_n(output, N, uint8_t{0});
	std::copy(encoded.begin(), encoded.end(), output);
	return true;
}

StgParamValue wireToParam(const kuf_stg::StgParamValue &wp) {
	StgParamValue p;
	p.type = static_cast<StgParamType>(wp.type_tag);
	if (wp.type_tag == 0 || wp.type_tag == 3) {
		p.intValue = std::get<int32_t>(wp.value);
	} else if (wp.type_tag == 1) {
		p.floatValue = std::get<float>(wp.value);
	} else if (wp.type_tag == 2) {
		const auto &sp = std::get<kuf_stg::StgStringParam>(wp.value);
		p.stringValue.assign(sp.value.begin(), sp.value.end());
	}
	return p;
}

StgScriptEntry wireToScript(const kuf_stg::StgCondition &wc) {
	StgScriptEntry entry;
	entry.typeId = wc.type_id;
	entry.params.reserve(wc.params.size());
	for (const auto &wp : wc.params) {
		entry.params.push_back(wireToParam(wp));
	}
	return entry;
}

StgScriptEntry wireToScript(const kuf_stg::StgAction &wa) {
	StgScriptEntry entry;
	entry.typeId = wa.type_id;
	entry.params.reserve(wa.params.size());
	for (const auto &wp : wa.params) {
		entry.params.push_back(wireToParam(wp));
	}
	return entry;
}

bool headerToWire(const StgHeader &h, kuf_stg::StgHeader &w,
		  std::optional<STGSaveError> &error) {
	w = h.wire_;
	return writeFixedText(h.mapFile, h.mapFileImage_, w.map_filename, false,
			      "header.mapFile", error) &&
	       writeFixedText(h.bitmapFile, h.bitmapFileImage_,
			      w.bitmap_filename, false, "header.bitmapFile",
			      error) &&
	       writeFixedText(h.defaultCameraFile, h.defaultCameraFileImage_,
			      w.default_camera, false,
			      "header.defaultCameraFile", error) &&
	       writeFixedText(h.userCameraFile, h.userCameraFileImage_,
			      w.user_camera, false, "header.userCameraFile",
			      error) &&
	       writeFixedText(h.settingsFile, h.settingsFileImage_,
			      w.settings_file, false, "header.settingsFile",
			      error) &&
	       writeFixedText(h.skyCloudEffects, h.skyCloudEffectsImage_,
			      w.sky_effects, false, "header.skyCloudEffects",
			      error) &&
	       writeFixedText(h.aiScriptFile, h.aiScriptFileImage_, w.ai_script,
			      false, "header.aiScriptFile", error) &&
	       writeFixedText(h.cubemapTexture, h.cubemapTextureImage_,
			      w.cubemap_texture, false, "header.cubemapTexture",
			      error);
}

bool unitToWire(const StgUnit &u, size_t index, kuf_stg::UnitBlock &w,
		std::optional<STGSaveError> &error) {
	w = u.wire_;
	if (!writeFixedText(u.unitName, u.unitNameImage_, w.name, true,
			    "units[" + std::to_string(index) + "].unitName",
			    error)) {
		return false;
	}
	w.unique_id = u.uniqueId;
	w.ucd = static_cast<uint8_t>(u.ucd);
	w.is_hero = u.isHero;
	w.is_enabled = u.isEnabled;
	w.leader_hp_override = u.leaderHpOverride;
	w.unit_hp_override = u.unitHpOverride;
	w.pos_x = u.positionX;
	w.pos_y = u.positionY;
	w.facing_direction = static_cast<uint8_t>(u.direction);

	w.leader_job_type = u.leaderJobType;
	w.leader_model_id = u.leaderModelId;
	w.leader_worldmap_id = u.leaderWorldmapId;
	w.leader_level = u.leaderLevel;
	for (int i = 0; i < 4; ++i) {
		w.leader_skills[i * 2] = u.leaderSkills[i].skillId;
		w.leader_skills[i * 2 + 1] = u.leaderSkills[i].level;
	}
	w.leader_abilities.assign(u.leaderAbilities.begin(),
				  u.leaderAbilities.end());

	w.officer_count = u.officerCount;

	w.officer1_job_type = u.officer1.jobType;
	w.officer1_model_id = u.officer1.modelId;
	w.officer1_worldmap_id = u.officer1.worldmapId;
	w.officer1_level = u.officer1.level;
	for (int i = 0; i < 4; ++i) {
		w.officer1_data[i * 2] = u.officer1.skills[i].skillId;
		w.officer1_data[i * 2 + 1] = u.officer1.skills[i].level;
	}
	for (int i = 0; i < 23; ++i) {
		std::memcpy(w.officer1_data + 8 + i * 4,
			    &u.officer1.abilities[i], 4);
	}

	w.officer2_job_type = u.officer2.jobType;
	w.officer2_model_id = u.officer2.modelId;
	w.officer2_worldmap_id = u.officer2.worldmapId;
	w.officer2_level = u.officer2.level;
	for (int i = 0; i < 4; ++i) {
		w.officer2_data[i * 2] = u.officer2.skills[i].skillId;
		w.officer2_data[i * 2 + 1] = u.officer2.skills[i].level;
	}
	for (int i = 0; i < 19; ++i) {
		std::memcpy(w.officer2_data + 8 + i * 4,
			    &u.officer2.abilities[i], 4);
	}

	w.animation_config = u.unitAnimConfig;
	w.grid_x = u.gridX;
	w.grid_y = u.gridY;
	w.troop_info_index = u.troopInfoIndex;
	w.formation_type = u.formationType;
	w.stat_overrides.assign(u.statOverrides.begin(), u.statOverrides.end());

	return true;
}

bool areaToWire(const StgArea &a, size_t index, kuf_stg::AreaEntry &w,
		std::optional<STGSaveError> &error) {
	w = a.wire_;
	if (!writeFixedText(
		a.description, a.descriptionImage_, w.description, true,
		"areas[" + std::to_string(index) + "].description", error)) {
		return false;
	}
	w.area_id = a.areaId;
	w.bound_x1 = a.boundX1;
	w.bound_y1 = a.boundY1;
	w.bound_x2 = a.boundX2;
	w.bound_y2 = a.boundY2;
	return true;
}

kuf_stg::StgParamValue domainParamToWire(const StgParamValue &p) {
	kuf_stg::StgParamValue w;
	w.type_tag = static_cast<uint32_t>(p.type);
	if (p.type == StgParamType::String) {
		kuf_stg::StgStringParam sp;
		sp.length = static_cast<uint32_t>(p.stringValue.size());
		sp.value.assign(p.stringValue.begin(), p.stringValue.end());
		w.value = sp;
	} else if (p.type == StgParamType::Float) {
		w.value = p.floatValue;
	} else {
		w.value = p.intValue;
	}
	return w;
}

bool variableToWire(const StgVariable &variable, size_t index,
		    kuf_stg::StgVariable &wire,
		    std::optional<STGSaveError> &error) {
	wire = variable.wire_;
	if (!writeFixedText(variable.name, variable.nameImage_, wire.name, true,
			    "variables[" + std::to_string(index) + "].name",
			    error)) {
		return false;
	}
	wire.variable_id = variable.variableId;
	wire.initial_value = domainParamToWire(variable.initialValue);
	return true;
}

bool eventToWire(const StgEvent &e, size_t blockIndex, size_t eventIndex,
		 kuf_stg::StgEvent &w, std::optional<STGSaveError> &error) {
	w = e.wire_;
	if (!writeFixedText(
		e.description, e.descriptionImage_, w.description, true,
		"eventBlocks[" + std::to_string(blockIndex) + "].events[" +
		    std::to_string(eventIndex) + "].description",
		error)) {
		return false;
	}
	w.event_id = e.eventId;
	w.condition_count = static_cast<uint32_t>(e.conditions.size());
	w.conditions.clear();
	for (const auto &cond : e.conditions) {
		kuf_stg::StgCondition wc;
		wc.type_id = cond.typeId;
		wc.param_count = static_cast<uint32_t>(cond.params.size());
		for (const auto &p : cond.params) {
			wc.params.push_back(domainParamToWire(p));
		}
		w.conditions.push_back(std::move(wc));
	}
	w.action_count = static_cast<uint32_t>(e.actions.size());
	w.actions.clear();
	for (const auto &act : e.actions) {
		kuf_stg::StgAction wa;
		wa.type_id = act.typeId;
		wa.param_count = static_cast<uint32_t>(act.params.size());
		for (const auto &p : act.params) {
			wa.params.push_back(domainParamToWire(p));
		}
		w.actions.push_back(std::move(wa));
	}
	return true;
}

} // namespace

bool StgFormat::load(std::span<const std::byte> data) {
	const auto *buf = reinterpret_cast<const uint8_t *>(data.data());
	size_t len = data.size();

	if (len < kStgHeaderSize) return false;

	try {
		size_t offset = 0;

		// Phase 1: Parse magic + header + units using cleave parsers.
		uint32_t magic;
		std::memcpy(&magic, buf, 4);
		if (magic != 0x3E9) return false;
		offset = 4;

		auto wireHeader = kuf_stg::StgHeader::parse(buf, len, offset);

		if (offset + 4 > len) return false;
		uint32_t unitCount;
		std::memcpy(&unitCount, buf + offset, 4);
		offset += 4;

		if (offset + static_cast<size_t>(unitCount) * kStgUnitSize >
		    len)
			return false;

		std::vector<kuf_stg::UnitBlock> wireUnits;
		wireUnits.reserve(unitCount);
		for (uint32_t i = 0; i < unitCount; ++i) {
			wireUnits.push_back(
			    kuf_stg::UnitBlock::parse(buf, len, offset));
		}

		// Convert header.
		header_.wire_ = wireHeader;
		header_.formatMagic = magic;
		header_.mapFileImage_ = UTF8TextImage(wireHeader.map_filename);
		header_.mapFile = header_.mapFileImage_.decoded;
		header_.bitmapFileImage_ =
		    UTF8TextImage(wireHeader.bitmap_filename);
		header_.bitmapFile = header_.bitmapFileImage_.decoded;
		header_.defaultCameraFileImage_ =
		    UTF8TextImage(wireHeader.default_camera);
		header_.defaultCameraFile =
		    header_.defaultCameraFileImage_.decoded;
		header_.userCameraFileImage_ =
		    UTF8TextImage(wireHeader.user_camera);
		header_.userCameraFile = header_.userCameraFileImage_.decoded;
		header_.settingsFileImage_ =
		    UTF8TextImage(wireHeader.settings_file);
		header_.settingsFile = header_.settingsFileImage_.decoded;
		header_.skyCloudEffectsImage_ =
		    UTF8TextImage(wireHeader.sky_effects);
		header_.skyCloudEffects = header_.skyCloudEffectsImage_.decoded;
		header_.aiScriptFileImage_ =
		    UTF8TextImage(wireHeader.ai_script);
		header_.aiScriptFile = header_.aiScriptFileImage_.decoded;
		header_.cubemapTextureImage_ =
		    UTF8TextImage(wireHeader.cubemap_texture);
		header_.cubemapTexture = header_.cubemapTextureImage_.decoded;
		header_.unitCount = unitCount;

		// Convert units.
		units_.clear();
		units_.resize(wireUnits.size());
		for (size_t i = 0; i < wireUnits.size(); ++i) {
			auto &unit = units_[i];
			const auto &wu = wireUnits[i];

			unit.wire_ = wu;

			unit.unitNameImage_ = CP949TextImage(wu.name);
			unit.unitName = unit.unitNameImage_.decoded;
			unit.uniqueId = wu.unique_id;
			unit.ucd = static_cast<UCD>(wu.ucd);
			unit.isHero = wu.is_hero;
			unit.isEnabled = wu.is_enabled;
			unit.leaderHpOverride = wu.leader_hp_override;
			unit.unitHpOverride = wu.unit_hp_override;
			unit.positionX = wu.pos_x;
			unit.positionY = wu.pos_y;
			unit.direction =
			    static_cast<Direction>(wu.facing_direction);

			unit.leaderJobType = wu.leader_job_type;
			unit.leaderModelId = wu.leader_model_id;
			unit.leaderWorldmapId = wu.leader_worldmap_id;
			unit.leaderLevel = wu.leader_level;

			for (int s = 0; s < 4; ++s) {
				unit.leaderSkills[s].skillId =
				    wu.leader_skills[s * 2];
				unit.leaderSkills[s].level =
				    wu.leader_skills[s * 2 + 1];
			}

			for (int a = 0;
			     a < 23 &&
			     a < static_cast<int>(wu.leader_abilities.size());
			     ++a) {
				unit.leaderAbilities[a] =
				    wu.leader_abilities[a];
			}

			unit.officerCount = wu.officer_count;

			unit.officer1.jobType = wu.officer1_job_type;
			unit.officer1.modelId = wu.officer1_model_id;
			unit.officer1.worldmapId = wu.officer1_worldmap_id;
			unit.officer1.level = wu.officer1_level;
			for (int s = 0; s < 4; ++s) {
				unit.officer1.skills[s].skillId =
				    wu.officer1_data[s * 2];
				unit.officer1.skills[s].level =
				    wu.officer1_data[s * 2 + 1];
			}
			for (int a = 0; a < 23; ++a) {
				std::memcpy(&unit.officer1.abilities[a],
					    wu.officer1_data + 8 + a * 4, 4);
			}

			unit.officer2.jobType = wu.officer2_job_type;
			unit.officer2.modelId = wu.officer2_model_id;
			unit.officer2.worldmapId = wu.officer2_worldmap_id;
			unit.officer2.level = wu.officer2_level;
			for (int s = 0; s < 4; ++s) {
				unit.officer2.skills[s].skillId =
				    wu.officer2_data[s * 2];
				unit.officer2.skills[s].level =
				    wu.officer2_data[s * 2 + 1];
			}
			for (int a = 0; a < 19; ++a) {
				std::memcpy(&unit.officer2.abilities[a],
					    wu.officer2_data + 8 + a * 4, 4);
			}

			unit.unitAnimConfig = wu.animation_config;
			unit.gridX = wu.grid_x;
			unit.gridY = wu.grid_y;
			unit.troopInfoIndex = wu.troop_info_index;
			unit.formationType = wu.formation_type;

			for (int f = 0;
			     f < 22 &&
			     f < static_cast<int>(wu.stat_overrides.size());
			     ++f) {
				unit.statOverrides[f] = wu.stat_overrides[f];
			}
		}

		// Phase 2: Try to parse tail (areas, variables, events,
		// footer).
		size_t tailStart = offset;
		tailParsed_ = false;
		rawTail_.clear();
		areas_.clear();
		variables_.clear();
		eventBlocks_.clear();
		footerEntries_.clear();

		if (tailStart < len) {
			try {
				// Areas.
				if (offset + 4 > len)
					throw std::runtime_error("truncated");
				uint32_t areaCount;
				std::memcpy(&areaCount, buf + offset, 4);
				offset += 4;

				areas_.reserve(areaCount);
				for (uint32_t i = 0; i < areaCount; ++i) {
					auto wa = kuf_stg::AreaEntry::parse(
					    buf, len, offset);
					StgArea area;
					area.wire_ = wa;
					area.descriptionImage_ =
					    CP949TextImage(wa.description);
					area.description =
					    area.descriptionImage_.decoded;
					area.areaId = wa.area_id;
					area.boundX1 = wa.bound_x1;
					area.boundY1 = wa.bound_y1;
					area.boundX2 = wa.bound_x2;
					area.boundY2 = wa.bound_y2;
					areas_.push_back(std::move(area));
				}

				// Variables.
				if (offset + 4 > len)
					throw std::runtime_error("truncated");
				uint32_t varCount;
				std::memcpy(&varCount, buf + offset, 4);
				offset += 4;

				variables_.reserve(varCount);
				for (uint32_t i = 0; i < varCount; ++i) {
					auto wv = kuf_stg::StgVariable::parse(
					    buf, len, offset);
					StgVariable var;
					var.wire_ = wv;
					var.nameImage_ =
					    CP949TextImage(wv.name);
					var.name = var.nameImage_.decoded;
					var.variableId = wv.variable_id;
					var.initialValue =
					    wireToParam(wv.initial_value);
					variables_.push_back(std::move(var));
				}

				// Event blocks.
				if (offset + 4 > len)
					throw std::runtime_error("truncated");
				uint32_t blockCount;
				std::memcpy(&blockCount, buf + offset, 4);
				offset += 4;

				eventBlocks_.reserve(blockCount);
				for (uint32_t i = 0; i < blockCount; ++i) {
					auto wb = kuf_stg::EventBlock::parse(
					    buf, len, offset);
					StgEventBlock block;
					block.blockHeader = wb.block_header;
					block.events.reserve(wb.events.size());

					for (const auto &we : wb.events) {
						StgEvent event;
						event.descriptionImage_ =
						    CP949TextImage(
							we.description);
						event.description =
						    event.descriptionImage_
							.decoded;
						event.eventId = we.event_id;

						event.conditions.reserve(
						    we.conditions.size());
						for (const auto &wc :
						     we.conditions) {
							event.conditions
							    .push_back(
								wireToScript(
								    wc));
						}

						event.actions.reserve(
						    we.actions.size());
						for (const auto &wa :
						     we.actions) {
							event.actions.push_back(
							    wireToScript(wa));
						}

						event.wire_ = we;
						event.modified = false;

						block.events.push_back(
						    std::move(event));
					}

					eventBlocks_.push_back(
					    std::move(block));
				}

				// Footer.
				if (offset + 4 > len)
					throw std::runtime_error("truncated");
				uint32_t footerCount;
				std::memcpy(&footerCount, buf + offset, 4);
				offset += 4;

				footerEntries_.reserve(footerCount);
				for (uint32_t i = 0; i < footerCount; ++i) {
					auto wf = kuf_stg::FooterEntry::parse(
					    buf, len, offset);
					footerEntries_.push_back(
					    {wf.slot_data_1, wf.slot_data_2});
				}

				tailParsed_ = true;
			} catch (...) {
				areas_.clear();
				variables_.clear();
				eventBlocks_.clear();
				footerEntries_.clear();
				rawTail_.assign(
				    reinterpret_cast<const std::byte *>(
					buf + tailStart),
				    reinterpret_cast<const std::byte *>(buf +
									len));
				tailParsed_ = false;
			}
		}

		version_ = GameVersion::Crusaders;
		return true;
	} catch (const std::exception &) {
		return false;
	}
}

std::vector<std::byte> StgFormat::save() const {
	auto result = trySave();
	return std::move(result.bytes);
}

STGSaveResult StgFormat::trySave() const {
	std::optional<STGSaveError> error;
	kuf_stg::StgHeader wireHeader;
	if (!headerToWire(header_, wireHeader, error)) {
		return {{}, std::move(error)};
	}

	if (tailParsed_) {
		kuf_stg::File file;
		file.magic = header_.formatMagic;
		file.header = std::move(wireHeader);

		for (size_t index = 0; index < units_.size(); ++index) {
			kuf_stg::UnitBlock wire;
			if (!unitToWire(units_[index], index, wire, error)) {
				return {{}, std::move(error)};
			}
			file.units.push_back(std::move(wire));
		}

		for (size_t index = 0; index < areas_.size(); ++index) {
			kuf_stg::AreaEntry wire;
			if (!areaToWire(areas_[index], index, wire, error)) {
				return {{}, std::move(error)};
			}
			file.areas.push_back(std::move(wire));
		}

		for (size_t index = 0; index < variables_.size(); ++index) {
			kuf_stg::StgVariable wire;
			if (!variableToWire(variables_[index], index, wire,
					    error)) {
				return {{}, std::move(error)};
			}
			file.variables.push_back(std::move(wire));
		}

		for (size_t blockIndex = 0; blockIndex < eventBlocks_.size();
		     ++blockIndex) {
			const auto &block = eventBlocks_[blockIndex];
			kuf_stg::EventBlock wb;
			wb.block_header = block.blockHeader;
			for (size_t eventIndex = 0;
			     eventIndex < block.events.size(); ++eventIndex) {
				kuf_stg::StgEvent wire;
				if (!eventToWire(block.events[eventIndex],
						 blockIndex, eventIndex, wire,
						 error)) {
					return {{}, std::move(error)};
				}
				wb.events.push_back(std::move(wire));
			}
			file.event_blocks.push_back(std::move(wb));
		}

		for (const auto &entry : footerEntries_) {
			file.footer_entries.push_back(
			    {entry.field1, entry.field2});
		}

		auto bytes = file.to_bytes();
		return {{reinterpret_cast<const std::byte *>(bytes.data()),
			 reinterpret_cast<const std::byte *>(bytes.data() +
							     bytes.size())},
			std::nullopt};
	}

	// Tail not parsed: emit header + units manually, append raw tail.
	std::vector<std::byte> data;

	// Magic.
	data.resize(4);
	std::memcpy(data.data(), &header_.formatMagic, 4);

	// Header.
	auto hdrBytes = wireHeader.to_bytes();
	data.insert(data.end(),
		    reinterpret_cast<const std::byte *>(hdrBytes.data()),
		    reinterpret_cast<const std::byte *>(hdrBytes.data() +
							hdrBytes.size()));

	// Unit count.
	uint32_t unitCount = static_cast<uint32_t>(units_.size());
	size_t pos = data.size();
	data.resize(pos + 4);
	std::memcpy(data.data() + pos, &unitCount, 4);

	// Units.
	for (size_t index = 0; index < units_.size(); ++index) {
		kuf_stg::UnitBlock wire;
		if (!unitToWire(units_[index], index, wire, error)) {
			return {{}, std::move(error)};
		}
		auto uBytes = wire.to_bytes();
		data.insert(data.end(),
			    reinterpret_cast<const std::byte *>(uBytes.data()),
			    reinterpret_cast<const std::byte *>(uBytes.data() +
								uBytes.size()));
	}

	data.insert(data.end(), rawTail_.begin(), rawTail_.end());
	return {std::move(data), std::nullopt};
}

size_t StgFormat::totalEventCount() const {
	size_t count = 0;
	for (const auto &block : eventBlocks_) {
		count += block.events.size();
	}
	return count;
}

std::vector<ValidationIssue> StgFormat::validate() const {
	std::vector<ValidationIssue> issues;

	for (size_t i = 0; i < units_.size(); ++i) {
		const auto &unit = units_[i];

		if (unit.unitName.empty()) {
			issues.push_back({Severity::Warning, "unitName",
					  "Unit has no name", i});
		}

		if (static_cast<uint8_t>(unit.ucd) > 3) {
			issues.push_back(
			    {Severity::Error, "ucd", "Invalid UCD value", i});
		}

		if (unit.leaderLevel == 0 || unit.leaderLevel > 99) {
			issues.push_back({Severity::Warning, "leaderLevel",
					  "Level outside typical range (1-99)",
					  i});
		}

		if (unit.leaderWorldmapId != 0xFF &&
		    unit.leaderWorldmapId > 20) {
			issues.push_back(
			    {Severity::Warning, "leaderWorldmapId",
			     "Worldmap ID may cause post-mission issues", i});
		}

		for (size_t j = i + 1; j < units_.size(); ++j) {
			if (units_[j].uniqueId == unit.uniqueId) {
				issues.push_back(
				    {Severity::Error, "uniqueId",
				     "Duplicate unique ID: " +
					 std::to_string(unit.uniqueId),
				     i});
				break;
			}
		}

		if (unit.officerCount > 2) {
			issues.push_back({Severity::Error, "officerCount",
					  "Officer count exceeds maximum of 2",
					  i});
		}
	}

	return issues;
}

} // namespace kuf
