#include "formats/stg_format.h"

#include "core/text_encoding.h"
#include "parsers/kuf_stg.h"

#include <cstring>

namespace kuf {

namespace {

StgParamValue wireToParam(const kuf_stg::StgParamValue& wp) {
    StgParamValue p;
    p.type = static_cast<StgParamType>(wp.type_tag);
    if (wp.type_tag == 0 || wp.type_tag == 3) {
        p.intValue = std::get<int32_t>(wp.value);
    } else if (wp.type_tag == 1) {
        p.floatValue = std::get<float>(wp.value);
    } else if (wp.type_tag == 2) {
        const auto& sp = std::get<kuf_stg::StgStringParam>(wp.value);
        p.stringValue.assign(sp.value.begin(), sp.value.end());
    }
    return p;
}

StgScriptEntry wireToScript(const kuf_stg::StgCondition& wc) {
    StgScriptEntry entry;
    entry.typeId = wc.type_id;
    entry.params.reserve(wc.params.size());
    for (const auto& wp : wc.params) {
        entry.params.push_back(wireToParam(wp));
    }
    return entry;
}

StgScriptEntry wireToScript(const kuf_stg::StgAction& wa) {
    StgScriptEntry entry;
    entry.typeId = wa.type_id;
    entry.params.reserve(wa.params.size());
    for (const auto& wp : wa.params) {
        entry.params.push_back(wireToParam(wp));
    }
    return entry;
}

kuf_stg::StgHeader headerToWire(const StgHeader& h) {
    kuf_stg::StgHeader w = h.wire_;
    w.map_filename = h.mapFile;
    w.bitmap_filename = h.bitmapFile;
    w.default_camera = h.defaultCameraFile;
    w.user_camera = h.userCameraFile;
    w.settings_file = h.settingsFile;
    w.sky_effects = h.skyCloudEffects;
    w.ai_script = h.aiScriptFile;
    w.cubemap_texture = h.cubemapTexture;
    return w;
}

kuf_stg::UnitBlock unitToWire(const StgUnit& u) {
    kuf_stg::UnitBlock w = u.wire_;
    w.name = utf8ToCp949(u.unitName);
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
    w.leader_abilities.assign(u.leaderAbilities.begin(), u.leaderAbilities.end());

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
        std::memcpy(w.officer1_data + 8 + i * 4, &u.officer1.abilities[i], 4);
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
        std::memcpy(w.officer2_data + 8 + i * 4, &u.officer2.abilities[i], 4);
    }

    w.animation_config = u.unitAnimConfig;
    w.grid_x = u.gridX;
    w.grid_y = u.gridY;
    w.troop_info_index = u.troopInfoIndex;
    w.formation_type = u.formationType;
    w.stat_overrides.assign(u.statOverrides.begin(), u.statOverrides.end());

    return w;
}

kuf_stg::AreaEntry areaToWire(const StgArea& a) {
    kuf_stg::AreaEntry w = a.wire_;
    w.description = utf8ToCp949(a.description);
    w.area_id = a.areaId;
    w.bound_x1 = a.boundX1;
    w.bound_y1 = a.boundY1;
    w.bound_x2 = a.boundX2;
    w.bound_y2 = a.boundY2;
    return w;
}

kuf_stg::StgParamValue domainParamToWire(const StgParamValue& p) {
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

kuf_stg::StgEvent eventToWire(const StgEvent& e) {
    kuf_stg::StgEvent w;
    w.description = utf8ToCp949(e.description);
    w.event_id = e.eventId;
    w.condition_count = static_cast<uint32_t>(e.conditions.size());
    for (const auto& cond : e.conditions) {
        kuf_stg::StgCondition wc;
        wc.type_id = cond.typeId;
        wc.param_count = static_cast<uint32_t>(cond.params.size());
        for (const auto& p : cond.params) {
            wc.params.push_back(domainParamToWire(p));
        }
        w.conditions.push_back(std::move(wc));
    }
    w.action_count = static_cast<uint32_t>(e.actions.size());
    for (const auto& act : e.actions) {
        kuf_stg::StgAction wa;
        wa.type_id = act.typeId;
        wa.param_count = static_cast<uint32_t>(act.params.size());
        for (const auto& p : act.params) {
            wa.params.push_back(domainParamToWire(p));
        }
        w.actions.push_back(std::move(wa));
    }
    return w;
}

} // namespace

bool StgFormat::load(std::span<const std::byte> data) {
    const auto* buf = reinterpret_cast<const uint8_t*>(data.data());
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

        if (offset + static_cast<size_t>(unitCount) * kStgUnitSize > len) return false;

        std::vector<kuf_stg::UnitBlock> wireUnits;
        wireUnits.reserve(unitCount);
        for (uint32_t i = 0; i < unitCount; ++i) {
            wireUnits.push_back(kuf_stg::UnitBlock::parse(buf, len, offset));
        }

        // Convert header.
        header_.wire_ = wireHeader;
        header_.formatMagic = magic;
        header_.mapFile = wireHeader.map_filename;
        header_.bitmapFile = wireHeader.bitmap_filename;
        header_.defaultCameraFile = wireHeader.default_camera;
        header_.userCameraFile = wireHeader.user_camera;
        header_.settingsFile = wireHeader.settings_file;
        header_.skyCloudEffects = wireHeader.sky_effects;
        header_.aiScriptFile = wireHeader.ai_script;
        header_.cubemapTexture = wireHeader.cubemap_texture;
        header_.unitCount = unitCount;

        // Convert units.
        units_.clear();
        units_.resize(wireUnits.size());
        for (size_t i = 0; i < wireUnits.size(); ++i) {
            auto& unit = units_[i];
            const auto& wu = wireUnits[i];

            unit.wire_ = wu;

            unit.unitName = cp949ToUtf8(wu.name);
            unit.uniqueId = wu.unique_id;
            unit.ucd = static_cast<UCD>(wu.ucd);
            unit.isHero = wu.is_hero;
            unit.isEnabled = wu.is_enabled;
            unit.leaderHpOverride = wu.leader_hp_override;
            unit.unitHpOverride = wu.unit_hp_override;
            unit.positionX = wu.pos_x;
            unit.positionY = wu.pos_y;
            unit.direction = static_cast<Direction>(wu.facing_direction);

            unit.leaderJobType = wu.leader_job_type;
            unit.leaderModelId = wu.leader_model_id;
            unit.leaderWorldmapId = wu.leader_worldmap_id;
            unit.leaderLevel = wu.leader_level;

            for (int s = 0; s < 4; ++s) {
                unit.leaderSkills[s].skillId = wu.leader_skills[s * 2];
                unit.leaderSkills[s].level = wu.leader_skills[s * 2 + 1];
            }

            for (int a = 0; a < 23 && a < static_cast<int>(wu.leader_abilities.size()); ++a) {
                unit.leaderAbilities[a] = wu.leader_abilities[a];
            }

            unit.officerCount = wu.officer_count;

            unit.officer1.jobType = wu.officer1_job_type;
            unit.officer1.modelId = wu.officer1_model_id;
            unit.officer1.worldmapId = wu.officer1_worldmap_id;
            unit.officer1.level = wu.officer1_level;
            for (int s = 0; s < 4; ++s) {
                unit.officer1.skills[s].skillId = wu.officer1_data[s * 2];
                unit.officer1.skills[s].level = wu.officer1_data[s * 2 + 1];
            }
            for (int a = 0; a < 23; ++a) {
                std::memcpy(&unit.officer1.abilities[a], wu.officer1_data + 8 + a * 4, 4);
            }

            unit.officer2.jobType = wu.officer2_job_type;
            unit.officer2.modelId = wu.officer2_model_id;
            unit.officer2.worldmapId = wu.officer2_worldmap_id;
            unit.officer2.level = wu.officer2_level;
            for (int s = 0; s < 4; ++s) {
                unit.officer2.skills[s].skillId = wu.officer2_data[s * 2];
                unit.officer2.skills[s].level = wu.officer2_data[s * 2 + 1];
            }
            for (int a = 0; a < 19; ++a) {
                std::memcpy(&unit.officer2.abilities[a], wu.officer2_data + 8 + a * 4, 4);
            }

            unit.unitAnimConfig = wu.animation_config;
            unit.gridX = wu.grid_x;
            unit.gridY = wu.grid_y;
            unit.troopInfoIndex = wu.troop_info_index;
            unit.formationType = wu.formation_type;

            for (int f = 0; f < 22 && f < static_cast<int>(wu.stat_overrides.size()); ++f) {
                unit.statOverrides[f] = wu.stat_overrides[f];
            }
        }

        // Phase 2: Try to parse tail (areas, variables, events, footer).
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
                if (offset + 4 > len) throw std::runtime_error("truncated");
                uint32_t areaCount;
                std::memcpy(&areaCount, buf + offset, 4);
                offset += 4;

                areas_.reserve(areaCount);
                for (uint32_t i = 0; i < areaCount; ++i) {
                    auto wa = kuf_stg::AreaEntry::parse(buf, len, offset);
                    StgArea area;
                    area.wire_ = wa;
                    area.description = cp949ToUtf8(wa.description);
                    area.areaId = wa.area_id;
                    area.boundX1 = wa.bound_x1;
                    area.boundY1 = wa.bound_y1;
                    area.boundX2 = wa.bound_x2;
                    area.boundY2 = wa.bound_y2;
                    areas_.push_back(std::move(area));
                }

                // Variables.
                if (offset + 4 > len) throw std::runtime_error("truncated");
                uint32_t varCount;
                std::memcpy(&varCount, buf + offset, 4);
                offset += 4;

                variables_.reserve(varCount);
                for (uint32_t i = 0; i < varCount; ++i) {
                    auto wv = kuf_stg::StgVariable::parse(buf, len, offset);
                    StgVariable var;
                    var.name = cp949ToUtf8(wv.name);
                    var.variableId = wv.variable_id;
                    var.initialValue = wireToParam(wv.initial_value);
                    variables_.push_back(std::move(var));
                }

                // Event blocks.
                if (offset + 4 > len) throw std::runtime_error("truncated");
                uint32_t blockCount;
                std::memcpy(&blockCount, buf + offset, 4);
                offset += 4;

                eventBlocks_.reserve(blockCount);
                for (uint32_t i = 0; i < blockCount; ++i) {
                    auto wb = kuf_stg::EventBlock::parse(buf, len, offset);
                    StgEventBlock block;
                    block.blockHeader = wb.block_header;
                    block.events.reserve(wb.events.size());

                    for (const auto& we : wb.events) {
                        StgEvent event;
                        event.description = cp949ToUtf8(we.description);
                        event.eventId = we.event_id;

                        event.conditions.reserve(we.conditions.size());
                        for (const auto& wc : we.conditions) {
                            event.conditions.push_back(wireToScript(wc));
                        }

                        event.actions.reserve(we.actions.size());
                        for (const auto& wa : we.actions) {
                            event.actions.push_back(wireToScript(wa));
                        }

                        event.wire_ = we;
                        event.modified = false;

                        block.events.push_back(std::move(event));
                    }

                    eventBlocks_.push_back(std::move(block));
                }

                // Footer.
                if (offset + 4 > len) throw std::runtime_error("truncated");
                uint32_t footerCount;
                std::memcpy(&footerCount, buf + offset, 4);
                offset += 4;

                footerEntries_.reserve(footerCount);
                for (uint32_t i = 0; i < footerCount; ++i) {
                    auto wf = kuf_stg::FooterEntry::parse(buf, len, offset);
                    footerEntries_.push_back({wf.slot_data_1, wf.slot_data_2});
                }

                tailParsed_ = true;
            } catch (...) {
                areas_.clear();
                variables_.clear();
                eventBlocks_.clear();
                footerEntries_.clear();
                rawTail_.assign(
                    reinterpret_cast<const std::byte*>(buf + tailStart),
                    reinterpret_cast<const std::byte*>(buf + len));
                tailParsed_ = false;
            }
        }

        version_ = GameVersion::Crusaders;
        return true;
    } catch (const std::exception&) {
        return false;
    }
}

std::vector<std::byte> StgFormat::save() const {
    if (tailParsed_) {
        kuf_stg::File file;
        file.magic = header_.formatMagic;
        file.header = headerToWire(header_);

        for (const auto& unit : units_) {
            file.units.push_back(unitToWire(unit));
        }

        for (const auto& area : areas_) {
            file.areas.push_back(areaToWire(area));
        }

        for (const auto& var : variables_) {
            kuf_stg::StgVariable wv;
            wv.name = utf8ToCp949(var.name);
            wv.variable_id = var.variableId;
            wv.initial_value = domainParamToWire(var.initialValue);
            file.variables.push_back(std::move(wv));
        }

        for (const auto& block : eventBlocks_) {
            kuf_stg::EventBlock wb;
            wb.block_header = block.blockHeader;
            for (const auto& event : block.events) {
                if (!event.modified) {
                    wb.events.push_back(event.wire_);
                } else {
                    wb.events.push_back(eventToWire(event));
                }
            }
            file.event_blocks.push_back(std::move(wb));
        }

        for (const auto& entry : footerEntries_) {
            file.footer_entries.push_back({entry.field1, entry.field2});
        }

        auto bytes = file.to_bytes();
        return {reinterpret_cast<const std::byte*>(bytes.data()),
                reinterpret_cast<const std::byte*>(bytes.data() + bytes.size())};
    }

    // Tail not parsed: emit header + units manually, append raw tail.
    std::vector<std::byte> data;

    // Magic.
    data.resize(4);
    std::memcpy(data.data(), &header_.formatMagic, 4);

    // Header.
    auto hdrBytes = headerToWire(header_).to_bytes();
    data.insert(data.end(),
        reinterpret_cast<const std::byte*>(hdrBytes.data()),
        reinterpret_cast<const std::byte*>(hdrBytes.data() + hdrBytes.size()));

    // Unit count.
    uint32_t unitCount = static_cast<uint32_t>(units_.size());
    size_t pos = data.size();
    data.resize(pos + 4);
    std::memcpy(data.data() + pos, &unitCount, 4);

    // Units.
    for (const auto& unit : units_) {
        auto uBytes = unitToWire(unit).to_bytes();
        data.insert(data.end(),
            reinterpret_cast<const std::byte*>(uBytes.data()),
            reinterpret_cast<const std::byte*>(uBytes.data() + uBytes.size()));
    }

    data.insert(data.end(), rawTail_.begin(), rawTail_.end());
    return data;
}

size_t StgFormat::totalEventCount() const {
    size_t count = 0;
    for (const auto& block : eventBlocks_) {
        count += block.events.size();
    }
    return count;
}

std::vector<ValidationIssue> StgFormat::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < units_.size(); ++i) {
        const auto& unit = units_[i];

        if (unit.unitName.empty()) {
            issues.push_back({
                Severity::Warning,
                "unitName",
                "Unit has no name",
                i
            });
        }

        if (static_cast<uint8_t>(unit.ucd) > 3) {
            issues.push_back({
                Severity::Error,
                "ucd",
                "Invalid UCD value",
                i
            });
        }

        if (unit.leaderLevel == 0 || unit.leaderLevel > 99) {
            issues.push_back({
                Severity::Warning,
                "leaderLevel",
                "Level outside typical range (1-99)",
                i
            });
        }

        if (unit.leaderWorldmapId != 0xFF && unit.leaderWorldmapId > 20) {
            issues.push_back({
                Severity::Warning,
                "leaderWorldmapId",
                "Worldmap ID may cause post-mission issues",
                i
            });
        }

        for (size_t j = i + 1; j < units_.size(); ++j) {
            if (units_[j].uniqueId == unit.uniqueId) {
                issues.push_back({
                    Severity::Error,
                    "uniqueId",
                    "Duplicate unique ID: " + std::to_string(unit.uniqueId),
                    i
                });
                break;
            }
        }

        if (unit.officerCount > 2) {
            issues.push_back({
                Severity::Error,
                "officerCount",
                "Officer count exceeds maximum of 2",
                i
            });
        }
    }

    return issues;
}

} // namespace kuf
