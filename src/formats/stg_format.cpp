#include "formats/stg_format.h"

#include "core/text_encoding.h"

#include <algorithm>
#include <cstring>

namespace kuf {

namespace {

template<typename T>
T readLE(const std::byte* data) {
    T value;
    std::memcpy(&value, data, sizeof(T));
    return value;
}

template<typename T>
void writeLE(std::byte* data, T value) {
    std::memcpy(data, &value, sizeof(T));
}

void appendLE(std::vector<std::byte>& out, uint32_t value) {
    size_t pos = out.size();
    out.resize(pos + 4);
    std::memcpy(out.data() + pos, &value, 4);
}

void appendLE(std::vector<std::byte>& out, int32_t value) {
    size_t pos = out.size();
    out.resize(pos + 4);
    std::memcpy(out.data() + pos, &value, 4);
}

void appendLE(std::vector<std::byte>& out, float value) {
    size_t pos = out.size();
    out.resize(pos + 4);
    std::memcpy(out.data() + pos, &value, 4);
}

void appendBytes(std::vector<std::byte>& out, const void* data, size_t len) {
    size_t pos = out.size();
    out.resize(pos + len);
    std::memcpy(out.data() + pos, data, len);
}

void appendZeros(std::vector<std::byte>& out, size_t count) {
    out.resize(out.size() + count, std::byte{0});
}

std::string readFixedString(const std::byte* data, size_t maxLen) {
    const char* str = reinterpret_cast<const char*>(data);
    size_t len = strnlen(str, maxLen);
    return std::string(str, len);
}

void writeFixedString(std::byte* data, size_t maxLen, const std::string& str) {
    std::memset(data, 0, maxLen);
    size_t copyLen = std::min(str.size(), maxLen - 1);
    std::memcpy(data, str.data(), copyLen);
}

} // namespace

void StgFormat::parseHeader(const std::byte* data) {
    std::memcpy(header_.rawData.data(), data, kStgHeaderSize);

    header_.formatMagic = readLE<uint32_t>(data + 0x000);
    header_.mapFile = readFixedString(data + 0x048, 64);
    header_.bitmapFile = readFixedString(data + 0x088, 64);
    header_.defaultCameraFile = readFixedString(data + 0x0C8, 64);
    header_.userCameraFile = readFixedString(data + 0x108, 64);
    header_.settingsFile = readFixedString(data + 0x148, 64);
    header_.skyCloudEffects = readFixedString(data + 0x188, 64);
    header_.aiScriptFile = readFixedString(data + 0x1C8, 64);
    header_.cubemapTexture = readFixedString(data + 0x20C, 64);
    header_.unitCount = readLE<uint32_t>(data + 0x270);
}

void StgFormat::patchHeader() const {
    std::byte* raw = const_cast<std::byte*>(header_.rawData.data());

    writeLE(raw + 0x000, header_.formatMagic);
    writeFixedString(raw + 0x048, 64, header_.mapFile);
    writeFixedString(raw + 0x088, 64, header_.bitmapFile);
    writeFixedString(raw + 0x0C8, 64, header_.defaultCameraFile);
    writeFixedString(raw + 0x108, 64, header_.userCameraFile);
    writeFixedString(raw + 0x148, 64, header_.settingsFile);
    writeFixedString(raw + 0x188, 64, header_.skyCloudEffects);
    writeFixedString(raw + 0x1C8, 64, header_.aiScriptFile);
    writeFixedString(raw + 0x20C, 64, header_.cubemapTexture);
    writeLE(raw + 0x270, header_.unitCount);
}

void StgFormat::parseUnit(StgUnit& unit, const std::byte* data) {
    std::memcpy(unit.rawData.data(), data, kStgUnitSize);

    // Core unit data (84 bytes starting at offset 0x00).
    unit.unitName = cp949ToUtf8(readFixedString(data + 0x00, 32));
    unit.uniqueId = readLE<uint32_t>(data + 0x20);
    unit.ucd = static_cast<UCD>(static_cast<uint8_t>(data[0x24]));
    unit.isHero = static_cast<uint8_t>(data[0x25]);
    unit.isEnabled = static_cast<uint8_t>(data[0x26]);
    unit.leaderHpOverride = readLE<float>(data + 0x28);
    unit.unitHpOverride = readLE<float>(data + 0x2C);
    unit.positionX = readLE<float>(data + 0x44);
    unit.positionY = readLE<float>(data + 0x48);
    unit.direction = static_cast<Direction>(static_cast<uint8_t>(data[0x4C]));

    // Leader configuration (108 bytes starting at offset 0x54).
    unit.leaderJobType = static_cast<uint8_t>(data[0x54]);
    unit.leaderModelId = static_cast<uint8_t>(data[0x55]);
    unit.leaderWorldmapId = static_cast<uint8_t>(data[0x56]);
    unit.leaderLevel = static_cast<uint8_t>(data[0x57]);

    // Skill slots (8 bytes at offset 0x58).
    for (int i = 0; i < 4; ++i) {
        unit.leaderSkills[i].skillId = static_cast<uint8_t>(data[0x58 + i * 2]);
        unit.leaderSkills[i].level = static_cast<uint8_t>(data[0x59 + i * 2]);
    }

    // Ability slots (92 bytes = 23 x int32 at offset 0x60).
    for (int i = 0; i < 23; ++i) {
        unit.leaderAbilities[i] = readLE<int32_t>(data + 0x60 + i * 4);
    }

    // Officer count at offset 0xBC.
    unit.officerCount = readLE<uint32_t>(data + 0xBC);

    // Officer 1 data (starts at offset 0xC0).
    unit.officer1.jobType = static_cast<uint8_t>(data[0xC0]);
    unit.officer1.modelId = static_cast<uint8_t>(data[0xC1]);
    unit.officer1.worldmapId = static_cast<uint8_t>(data[0xC2]);
    unit.officer1.level = static_cast<uint8_t>(data[0xC3]);
    for (int i = 0; i < 4; ++i) {
        unit.officer1.skills[i].skillId = static_cast<uint8_t>(data[0xC4 + i * 2]);
        unit.officer1.skills[i].level = static_cast<uint8_t>(data[0xC5 + i * 2]);
    }
    for (int i = 0; i < 23; ++i) {
        unit.officer1.abilities[i] = readLE<int32_t>(data + 0xCC + i * 4);
    }

    // Officer 2 data (starts at offset 0x128).
    unit.officer2.jobType = static_cast<uint8_t>(data[0x128]);
    unit.officer2.modelId = static_cast<uint8_t>(data[0x129]);
    unit.officer2.worldmapId = static_cast<uint8_t>(data[0x12A]);
    unit.officer2.level = static_cast<uint8_t>(data[0x12B]);
    for (int i = 0; i < 4; ++i) {
        unit.officer2.skills[i].skillId = static_cast<uint8_t>(data[0x12C + i * 2]);
        unit.officer2.skills[i].level = static_cast<uint8_t>(data[0x12D + i * 2]);
    }
    for (int i = 0; i < 19; ++i) {
        unit.officer2.abilities[i] = readLE<int32_t>(data + 0x134 + i * 4);
    }

    // Unit configuration (160 bytes starting at offset 0x180).
    unit.unitAnimConfig = readLE<uint32_t>(data + 0x18C);
    unit.gridX = readLE<uint32_t>(data + 0x190);
    unit.gridY = readLE<uint32_t>(data + 0x194);
    unit.troopInfoIndex = readLE<int32_t>(data + 0x1C0);
    unit.formationType = readLE<uint32_t>(data + 0x1C4);

    // Stat overrides: 22 floats at offset 0x1C8.
    for (int i = 0; i < 22; ++i) {
        unit.statOverrides[i] = readLE<float>(data + 0x1C8 + i * 4);
    }
}

void StgFormat::patchUnit(StgUnit& unit) const {
    std::byte* raw = unit.rawData.data();

    // Core unit data.
    writeFixedString(raw + 0x00, 32, utf8ToCp949(unit.unitName));
    writeLE(raw + 0x20, unit.uniqueId);
    raw[0x24] = static_cast<std::byte>(unit.ucd);
    raw[0x25] = static_cast<std::byte>(unit.isHero);
    raw[0x26] = static_cast<std::byte>(unit.isEnabled);
    writeLE(raw + 0x28, unit.leaderHpOverride);
    writeLE(raw + 0x2C, unit.unitHpOverride);
    writeLE(raw + 0x44, unit.positionX);
    writeLE(raw + 0x48, unit.positionY);
    raw[0x4C] = static_cast<std::byte>(unit.direction);

    // Leader configuration.
    raw[0x54] = static_cast<std::byte>(unit.leaderJobType);
    raw[0x55] = static_cast<std::byte>(unit.leaderModelId);
    raw[0x56] = static_cast<std::byte>(unit.leaderWorldmapId);
    raw[0x57] = static_cast<std::byte>(unit.leaderLevel);

    for (int i = 0; i < 4; ++i) {
        raw[0x58 + i * 2] = static_cast<std::byte>(unit.leaderSkills[i].skillId);
        raw[0x59 + i * 2] = static_cast<std::byte>(unit.leaderSkills[i].level);
    }

    for (int i = 0; i < 23; ++i) {
        writeLE(raw + 0x60 + i * 4, unit.leaderAbilities[i]);
    }

    writeLE(raw + 0xBC, unit.officerCount);

    // Officer 1.
    raw[0xC0] = static_cast<std::byte>(unit.officer1.jobType);
    raw[0xC1] = static_cast<std::byte>(unit.officer1.modelId);
    raw[0xC2] = static_cast<std::byte>(unit.officer1.worldmapId);
    raw[0xC3] = static_cast<std::byte>(unit.officer1.level);
    for (int i = 0; i < 4; ++i) {
        raw[0xC4 + i * 2] = static_cast<std::byte>(unit.officer1.skills[i].skillId);
        raw[0xC5 + i * 2] = static_cast<std::byte>(unit.officer1.skills[i].level);
    }
    for (int i = 0; i < 23; ++i) {
        writeLE(raw + 0xCC + i * 4, unit.officer1.abilities[i]);
    }

    // Officer 2.
    raw[0x128] = static_cast<std::byte>(unit.officer2.jobType);
    raw[0x129] = static_cast<std::byte>(unit.officer2.modelId);
    raw[0x12A] = static_cast<std::byte>(unit.officer2.worldmapId);
    raw[0x12B] = static_cast<std::byte>(unit.officer2.level);
    for (int i = 0; i < 4; ++i) {
        raw[0x12C + i * 2] = static_cast<std::byte>(unit.officer2.skills[i].skillId);
        raw[0x12D + i * 2] = static_cast<std::byte>(unit.officer2.skills[i].level);
    }
    for (int i = 0; i < 19; ++i) {
        writeLE(raw + 0x134 + i * 4, unit.officer2.abilities[i]);
    }

    // Unit configuration.
    writeLE(raw + 0x18C, unit.unitAnimConfig);
    writeLE(raw + 0x190, unit.gridX);
    writeLE(raw + 0x194, unit.gridY);
    writeLE(raw + 0x1C0, unit.troopInfoIndex);
    writeLE(raw + 0x1C4, unit.formationType);

    for (int i = 0; i < 22; ++i) {
        writeLE(raw + 0x1C8 + i * 4, unit.statOverrides[i]);
    }
}

bool StgFormat::load(std::span<const std::byte> data) {
    if (data.size() < kStgHeaderSize) {
        return false;
    }

    parseHeader(data.data());

    uint32_t count = header_.unitCount;
    size_t expectedMinSize = kStgHeaderSize + count * kStgUnitSize;
    if (data.size() < expectedMinSize) {
        return false;
    }

    units_.clear();
    units_.resize(count);

    const std::byte* ptr = data.data() + kStgHeaderSize;
    for (uint32_t i = 0; i < count; ++i) {
        parseUnit(units_[i], ptr);
        ptr += kStgUnitSize;
    }

    size_t tailOffset = kStgHeaderSize + count * kStgUnitSize;
    size_t tailSize = data.size() - tailOffset;
    if (tailSize > 0) {
        if (!parseTail(data.data() + tailOffset, tailSize)) {
            rawTail_.assign(data.begin() + tailOffset, data.end());
            tailParsed_ = false;
        }
    } else {
        tailParsed_ = false;
    }

    version_ = GameVersion::Crusaders;
    return true;
}

std::vector<std::byte> StgFormat::save() const {
    patchHeader();
    for (auto& unit : const_cast<std::vector<StgUnit>&>(units_)) {
        patchUnit(unit);
    }

    std::vector<std::byte> data;

    // Write header.
    data.insert(data.end(), header_.rawData.begin(), header_.rawData.end());

    // Write units.
    for (const auto& unit : units_) {
        data.insert(data.end(), unit.rawData.begin(), unit.rawData.end());
    }

    if (!tailParsed_) {
        data.insert(data.end(), rawTail_.begin(), rawTail_.end());
        return data;
    }

    // Write parsed tail sections.
    serializeAreaIds(data);
    serializeVariables(data);
    serializeEventBlocks(data);
    serializeFooter(data);

    return data;
}

StgParamValue StgFormat::readParamValue(const std::byte* data, size_t& offset, size_t limit) const {
    StgParamValue val;
    if (offset + 4 > limit) {
        return val;
    }

    val.type = static_cast<StgParamType>(readLE<uint32_t>(data + offset));
    offset += 4;

    if (val.type == StgParamType::String) {
        if (offset + 4 > limit) return val;
        uint32_t slen = readLE<uint32_t>(data + offset);
        offset += 4;
        if (offset + slen > limit) return val;
        val.stringValue = std::string(reinterpret_cast<const char*>(data + offset), slen);
        offset += slen;
    } else if (val.type == StgParamType::Float) {
        if (offset + 4 > limit) return val;
        val.floatValue = readLE<float>(data + offset);
        offset += 4;
    } else {
        // Int or Enum — both are 4-byte int32.
        if (offset + 4 > limit) return val;
        val.intValue = readLE<int32_t>(data + offset);
        offset += 4;
    }

    return val;
}

void StgFormat::serializeParamValue(std::vector<std::byte>& out, const StgParamValue& val) const {
    appendLE(out, static_cast<uint32_t>(val.type));

    if (val.type == StgParamType::String) {
        appendLE(out, static_cast<uint32_t>(val.stringValue.size()));
        appendBytes(out, val.stringValue.data(), val.stringValue.size());
    } else if (val.type == StgParamType::Float) {
        appendLE(out, val.floatValue);
    } else {
        appendLE(out, val.intValue);
    }
}

size_t StgFormat::parseAreaIds(const std::byte* data, size_t tailSize, size_t offset) {
    if (offset + 4 > tailSize) return SIZE_MAX;
    uint32_t areaCount = readLE<uint32_t>(data + offset);
    size_t areaSection = 4 + static_cast<size_t>(areaCount) * kStgAreaIdEntrySize;
    if (offset + areaSection > tailSize) return SIZE_MAX;
    offset += 4;

    areas_.clear();
    areas_.reserve(areaCount);

    for (uint32_t i = 0; i < areaCount; ++i) {
        StgArea area;
        const std::byte* entry = data + offset;
        std::memcpy(area.rawData.data(), entry, kStgAreaIdEntrySize);

        area.description = readFixedString(entry + 0x00, 32);
        area.areaId = readLE<uint32_t>(entry + 0x40);
        area.boundX1 = readLE<float>(entry + 0x44);
        area.boundY1 = readLE<float>(entry + 0x48);
        area.boundX2 = readLE<float>(entry + 0x4C);
        area.boundY2 = readLE<float>(entry + 0x50);

        areas_.push_back(std::move(area));
        offset += kStgAreaIdEntrySize;
    }

    return offset;
}

size_t StgFormat::parseVariables(const std::byte* data, size_t tailSize, size_t offset) {
    if (offset + 4 > tailSize) return SIZE_MAX;
    uint32_t varCount = readLE<uint32_t>(data + offset);
    offset += 4;

    variables_.clear();
    variables_.reserve(varCount);

    for (uint32_t i = 0; i < varCount; ++i) {
        StgVariable var;

        // Fixed 64-byte name.
        if (offset + kStgVariableNameSize > tailSize) return SIZE_MAX;
        var.name = readFixedString(data + offset, kStgVariableNameSize);
        offset += kStgVariableNameSize;

        // Variable ID (4 bytes).
        if (offset + 4 > tailSize) return SIZE_MAX;
        var.variableId = readLE<uint32_t>(data + offset);
        offset += 4;

        // Typed initial value via ReadSTGParamValue.
        var.initialValue = readParamValue(data, offset, tailSize);

        variables_.push_back(std::move(var));
    }

    return offset;
}

size_t StgFormat::parseEventBlocks(const std::byte* data, size_t tailSize, size_t offset) {
    if (offset + 4 > tailSize) return SIZE_MAX;
    uint32_t blockCount = readLE<uint32_t>(data + offset);
    offset += 4;

    eventBlocks_.clear();
    eventBlocks_.reserve(blockCount);

    for (uint32_t b = 0; b < blockCount; ++b) {
        StgEventBlock block;

        // Block header (4 bytes).
        if (offset + 4 > tailSize) return SIZE_MAX;
        block.blockHeader = readLE<uint32_t>(data + offset);
        offset += 4;

        // Event count (4 bytes).
        if (offset + 4 > tailSize) return SIZE_MAX;
        uint32_t eventCount = readLE<uint32_t>(data + offset);
        offset += 4;

        block.events.reserve(eventCount);

        for (uint32_t e = 0; e < eventCount; ++e) {
            StgEvent event;
            size_t eventStart = offset;

            // Description (64 bytes).
            if (offset + kStgEventDescriptionSize > tailSize) return SIZE_MAX;
            event.description = readFixedString(data + offset, kStgEventDescriptionSize);
            offset += kStgEventDescriptionSize;

            // Event ID (4 bytes).
            if (offset + 4 > tailSize) return SIZE_MAX;
            event.eventId = readLE<uint32_t>(data + offset);
            offset += 4;

            // Condition count (4 bytes).
            if (offset + 4 > tailSize) return SIZE_MAX;
            uint32_t condCount = readLE<uint32_t>(data + offset);
            offset += 4;

            event.conditions.reserve(condCount);
            for (uint32_t c = 0; c < condCount; ++c) {
                StgScriptEntry cond;

                // Type ID (4 bytes).
                if (offset + 4 > tailSize) return SIZE_MAX;
                cond.typeId = readLE<uint32_t>(data + offset);
                offset += 4;

                // Param count (4 bytes).
                if (offset + 4 > tailSize) return SIZE_MAX;
                uint32_t paramCount = readLE<uint32_t>(data + offset);
                offset += 4;

                cond.params.reserve(paramCount);
                for (uint32_t p = 0; p < paramCount; ++p) {
                    cond.params.push_back(readParamValue(data, offset, tailSize));
                }

                event.conditions.push_back(std::move(cond));
            }

            // Action count (4 bytes).
            if (offset + 4 > tailSize) return SIZE_MAX;
            uint32_t actCount = readLE<uint32_t>(data + offset);
            offset += 4;

            event.actions.reserve(actCount);
            for (uint32_t a = 0; a < actCount; ++a) {
                StgScriptEntry act;

                // Type ID (4 bytes).
                if (offset + 4 > tailSize) return SIZE_MAX;
                act.typeId = readLE<uint32_t>(data + offset);
                offset += 4;

                // Param count (4 bytes).
                if (offset + 4 > tailSize) return SIZE_MAX;
                uint32_t paramCount = readLE<uint32_t>(data + offset);
                offset += 4;

                act.params.reserve(paramCount);
                for (uint32_t p = 0; p < paramCount; ++p) {
                    act.params.push_back(readParamValue(data, offset, tailSize));
                }

                event.actions.push_back(std::move(act));
            }

            // Store raw bytes for unmodified round-trip.
            event.rawData.assign(data + eventStart, data + offset);
            block.events.push_back(std::move(event));
        }

        eventBlocks_.push_back(std::move(block));
    }

    return offset;
}

size_t StgFormat::parseFooter(const std::byte* data, size_t tailSize, size_t offset) {
    if (offset + 4 > tailSize) return SIZE_MAX;
    uint32_t footerCount = readLE<uint32_t>(data + offset);
    offset += 4;

    footerEntries_.clear();
    footerEntries_.reserve(footerCount);

    if (offset + static_cast<size_t>(footerCount) * 8 > tailSize) return SIZE_MAX;

    for (uint32_t i = 0; i < footerCount; ++i) {
        StgFooterEntry entry;
        entry.field1 = readLE<uint32_t>(data + offset);
        entry.field2 = readLE<uint32_t>(data + offset + 4);
        footerEntries_.push_back(entry);
        offset += 8;
    }

    return offset;
}

bool StgFormat::parseTail(const std::byte* data, size_t tailSize) {
    size_t offset = 0;

    // AreaIDs section.
    offset = parseAreaIds(data, tailSize, offset);
    if (offset == SIZE_MAX) return false;

    // Variables section.
    offset = parseVariables(data, tailSize, offset);
    if (offset == SIZE_MAX) return false;

    // Event blocks section.
    offset = parseEventBlocks(data, tailSize, offset);
    if (offset == SIZE_MAX) return false;

    // Footer section.
    offset = parseFooter(data, tailSize, offset);
    if (offset == SIZE_MAX) return false;

    // Validate total consumed == tailSize.
    if (offset != tailSize) return false;

    tailParsed_ = true;
    return true;
}

void StgFormat::serializeAreaIds(std::vector<std::byte>& out) const {
    appendLE(out, static_cast<uint32_t>(areas_.size()));

    for (const auto& area : areas_) {
        // Patch known fields into rawData, then write the full 84 bytes.
        std::array<std::byte, 84> patched = area.rawData;
        writeFixedString(patched.data() + 0x00, 32, area.description);
        writeLE(patched.data() + 0x40, area.areaId);
        writeLE(patched.data() + 0x44, area.boundX1);
        writeLE(patched.data() + 0x48, area.boundY1);
        writeLE(patched.data() + 0x4C, area.boundX2);
        writeLE(patched.data() + 0x50, area.boundY2);
        appendBytes(out, patched.data(), patched.size());
    }
}

void StgFormat::serializeVariables(std::vector<std::byte>& out) const {
    appendLE(out, static_cast<uint32_t>(variables_.size()));

    for (const auto& var : variables_) {
        // Fixed 64-byte name.
        size_t nameStart = out.size();
        appendZeros(out, kStgVariableNameSize);
        size_t copyLen = std::min(var.name.size(), kStgVariableNameSize - 1);
        std::memcpy(out.data() + nameStart, var.name.data(), copyLen);

        // Variable ID.
        appendLE(out, var.variableId);

        // Typed initial value.
        serializeParamValue(out, var.initialValue);
    }
}

void StgFormat::serializeEventBlocks(std::vector<std::byte>& out) const {
    appendLE(out, static_cast<uint32_t>(eventBlocks_.size()));

    for (const auto& block : eventBlocks_) {
        // Block header.
        appendLE(out, block.blockHeader);

        // Event count.
        appendLE(out, static_cast<uint32_t>(block.events.size()));

        for (const auto& event : block.events) {
            if (!event.modified && !event.rawData.empty()) {
                // Unmodified event — emit raw bytes for byte-identical round-trip.
                out.insert(out.end(), event.rawData.begin(), event.rawData.end());
                continue;
            }

            // Description (64 bytes).
            size_t descStart = out.size();
            appendZeros(out, kStgEventDescriptionSize);
            size_t copyLen = std::min(event.description.size(), kStgEventDescriptionSize - 1);
            std::memcpy(out.data() + descStart, event.description.data(), copyLen);

            // Event ID.
            appendLE(out, event.eventId);

            // Conditions.
            appendLE(out, static_cast<uint32_t>(event.conditions.size()));
            for (const auto& cond : event.conditions) {
                appendLE(out, cond.typeId);
                appendLE(out, static_cast<uint32_t>(cond.params.size()));
                for (const auto& param : cond.params) {
                    serializeParamValue(out, param);
                }
            }

            // Actions.
            appendLE(out, static_cast<uint32_t>(event.actions.size()));
            for (const auto& act : event.actions) {
                appendLE(out, act.typeId);
                appendLE(out, static_cast<uint32_t>(act.params.size()));
                for (const auto& param : act.params) {
                    serializeParamValue(out, param);
                }
            }
        }
    }
}

void StgFormat::serializeFooter(std::vector<std::byte>& out) const {
    appendLE(out, static_cast<uint32_t>(footerEntries_.size()));

    for (const auto& entry : footerEntries_) {
        appendLE(out, entry.field1);
        appendLE(out, entry.field2);
    }
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

        // Check for duplicate unique IDs.
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

        // Validate officer count.
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
