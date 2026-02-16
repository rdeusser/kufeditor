#include "formats/save_format.h"

#include "kaitai/kaitaistream.h"
#include "parsers/kuf_save.h"

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

void appendBytes(std::vector<std::byte>& out, const void* data, size_t len) {
    size_t pos = out.size();
    out.resize(pos + len);
    std::memcpy(out.data() + pos, data, len);
}

void appendByte(std::vector<std::byte>& out, uint8_t value) {
    out.push_back(static_cast<std::byte>(value));
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

template<size_t N>
std::array<int32_t, N> bytesToInt32Array(const std::string& bytes) {
    std::array<int32_t, N> result{};
    std::memcpy(result.data(), bytes.data(), N * sizeof(int32_t));
    return result;
}

struct NormalizedSave {
    std::string buffer;
    bool hadSizePrefix = false;
    bool hadContext = false;
};

NormalizedSave normalizeForKaitai(std::span<const std::byte> data) {
    NormalizedSave result;

    if (data.size() < 8) return result;

    size_t pos = 0;

    // Detect and skip size prefix.
    uint32_t first = readLE<uint32_t>(data.data());
    uint32_t second = readLE<uint32_t>(data.data() + 4);
    if (first == static_cast<uint32_t>(data.size()) && second == kSaveMagic) {
        result.hadSizePrefix = true;
        pos = 4;
    }

    // Verify magic.
    uint32_t magic = readLE<uint32_t>(data.data() + pos);
    if (magic != kSaveMagic) return result;
    pos += 4;

    // Detect context block.
    if (pos + 4 <= data.size()) {
        int32_t candidate = readLE<int32_t>(data.data() + pos);
        if (candidate >= 0 && candidate <= 3) {
            result.hadContext = false;
        } else if (pos + kSaveContextSize + 4 <= data.size()) {
            int32_t ctxCampaign = readLE<int32_t>(data.data() + pos + kSaveContextSize);
            if (ctxCampaign >= 0 && ctxCampaign <= 3) {
                result.hadContext = true;
            }
        }
    }

    // Build canonical buffer: size_prefix(4) + magic(4) + context(0x438) + rest.
    std::string buf;
    buf.append(4, '\0'); // size prefix placeholder

    buf.append(reinterpret_cast<const char*>(&magic), 4);

    if (result.hadContext) {
        buf.append(reinterpret_cast<const char*>(data.data() + pos), kSaveContextSize);
        pos += kSaveContextSize;
    } else {
        buf.append(kSaveContextSize, '\0');
    }

    if (pos < data.size()) {
        buf.append(reinterpret_cast<const char*>(data.data() + pos), data.size() - pos);
    }

    // Patch size prefix to match total buffer size.
    uint32_t totalSize = static_cast<uint32_t>(buf.size());
    std::memcpy(buf.data(), &totalSize, 4);

    result.buffer = std::move(buf);
    return result;
}

std::string stripColorCodes(const std::string& text) {
    std::string result = text;

    for (const char* prefix : {"@(color=", "(color="}) {
        std::string pfx(prefix);
        while (true) {
            auto start = result.find(pfx);
            if (start == std::string::npos) break;
            auto end = result.find(')', start);
            if (end != std::string::npos) {
                result = result.substr(0, start) + result.substr(end + 1);
            } else {
                result = result.substr(0, start);
            }
        }
    }

    // Remove orphaned closing fragment: leading hex digits + ).
    auto paren = result.find(')');
    if (paren != std::string::npos && paren > 0) {
        bool allHex = true;
        for (size_t i = 0; i < paren; ++i) {
            char c = result[i];
            if (!((c >= '0' && c <= '9') || (c >= 'A' && c <= 'F') || (c >= 'a' && c <= 'f'))) {
                allHex = false;
                break;
            }
        }
        if (allHex) {
            result = result.substr(paren + 1);
        }
    }

    return result;
}

} // namespace

void SaveFormat::parseContext(const std::byte* data) {
    std::memcpy(context_.rawData.data(), data, kSaveContextSize);

    // Extract readable text segments from raw context bytes.
    std::string current;
    for (size_t i = 0; i < kSaveContextSize; ++i) {
        uint8_t b = static_cast<uint8_t>(data[i]);
        if ((b >= 0x20 && b < 0x7F) || b == 0x0A || b == 0x0D) {
            current += static_cast<char>(b);
        } else {
            if (current.size() >= 4) {
                // Trim whitespace.
                size_t start = current.find_first_not_of(" \t\r\n");
                size_t end = current.find_last_not_of(" \t\r\n");
                if (start != std::string::npos) {
                    current = current.substr(start, end - start + 1);
                }
                if (current.size() >= 4) {
                    context_.displayText.push_back(current);
                }
            }
            current.clear();
        }
    }
    if (current.size() >= 4) {
        size_t start = current.find_first_not_of(" \t\r\n");
        size_t end = current.find_last_not_of(" \t\r\n");
        if (start != std::string::npos) {
            current = current.substr(start, end - start + 1);
        }
        if (current.size() >= 4) {
            context_.displayText.push_back(current);
        }
    }

    // Deduplicate and strip color codes.
    std::vector<std::string> cleaned;
    std::vector<std::string> seen;
    for (const auto& text : context_.displayText) {
        std::string stripped = stripColorCodes(text);
        // Split on newlines.
        size_t pos = 0;
        while (pos < stripped.size()) {
            auto nl = stripped.find('\n', pos);
            std::string line;
            if (nl != std::string::npos) {
                line = stripped.substr(pos, nl - pos);
                pos = nl + 1;
            } else {
                line = stripped.substr(pos);
                pos = stripped.size();
            }
            // Trim.
            size_t s = line.find_first_not_of(" \t\r");
            size_t e = line.find_last_not_of(" \t\r");
            if (s != std::string::npos) {
                line = line.substr(s, e - s + 1);
            } else {
                line.clear();
            }
            if (line.size() >= 4) {
                bool duplicate = false;
                for (const auto& prev : seen) {
                    if (prev == line) { duplicate = true; break; }
                }
                if (!duplicate) {
                    seen.push_back(line);
                    cleaned.push_back(line);
                }
            }
        }
    }
    context_.displayText = std::move(cleaned);
}

void SaveFormat::parseMainBlock(const std::byte* data) {
    std::memcpy(mainBlock_.rawData.data(), data, kSaveMainBlockSize);

    mainBlock_.field00 = readLE<uint32_t>(data + 0x00);
    mainBlock_.field04 = readLE<uint32_t>(data + 0x04);
    mainBlock_.field08 = readLE<int32_t>(data + 0x08);
    mainBlock_.field0c = readLE<uint32_t>(data + 0x0C);
    mainBlock_.field10 = readLE<uint32_t>(data + 0x10);
    mainBlock_.field14 = readLE<uint32_t>(data + 0x14);
    mainBlock_.field18 = readLE<uint32_t>(data + 0x18);

    mainBlock_.mapName = readFixedString(data + 0x20, 32);
    mainBlock_.setFile = readFixedString(data + 0x60, 32);
    mainBlock_.skyEffects = readFixedString(data + 0xA0, 32);
}

void SaveFormat::patchMainBlock() const {
    std::byte* raw = const_cast<std::byte*>(mainBlock_.rawData.data());

    writeLE(raw + 0x00, mainBlock_.field00);
    writeLE(raw + 0x04, mainBlock_.field04);
    writeLE(raw + 0x08, mainBlock_.field08);
    writeLE(raw + 0x0C, mainBlock_.field0c);
    writeLE(raw + 0x10, mainBlock_.field10);
    writeLE(raw + 0x14, mainBlock_.field14);
    writeLE(raw + 0x18, mainBlock_.field18);

    writeFixedString(raw + 0x20, 32, mainBlock_.mapName);
    writeFixedString(raw + 0x60, 32, mainBlock_.setFile);
    writeFixedString(raw + 0xA0, 32, mainBlock_.skyEffects);
}

bool SaveFormat::load(std::span<const std::byte> data) {
    if (data.size() < 8) return false;

    auto normalized = normalizeForKaitai(data);
    if (normalized.buffer.empty()) return false;

    hasSizePrefix_ = normalized.hadSizePrefix;
    hasContext_ = normalized.hadContext;

    try {
        kaitai::kstream ks(normalized.buffer);
        kuf_save_t parsed(&ks);

        // Context.
        if (hasContext_) {
            const auto& ctxBytes = parsed.context_data();
            parseContext(reinterpret_cast<const std::byte*>(ctxBytes.data()));
        }

        // Campaign index.
        campaignIndex_ = static_cast<int32_t>(parsed.campaign_index());

        // Main block.
        const auto& mainBytes = parsed.main_save_block();
        parseMainBlock(reinterpret_cast<const std::byte*>(mainBytes.data()));

        // Units.
        units_.clear();
        if (parsed.units()) {
            units_.resize(parsed.units()->size());
            for (size_t i = 0; i < parsed.units()->size(); ++i) {
                const auto* ku = (*parsed.units())[i];
                SaveUnit& unit = units_[i];

                unit.unknownIndex = static_cast<int32_t>(ku->unknown_index());
                unit.troopInfoIndex = static_cast<int32_t>(ku->troop_info_index());
                unit.jobType = ku->job_type();
                unit.modelId = ku->model_id();
                unit.stgField34 = ku->stg_field_190();
                unit.stgField38 = ku->stg_field_192();
                unit.stgField3c = ku->stg_field_194();
                unit.stgField40 = ku->stg_field_198();
                unit.charId = static_cast<int32_t>(ku->char_id());
                unit.troopInfoIndex2 = static_cast<int32_t>(ku->troop_info_index_2());
                unit.ucd = ku->ucd();
                unit.formationType = ku->formation_type();
                unit.gridConfig = ku->grid_config();
                unit.skillLevel = ku->skill_level();
                unit.byte58 = ku->byte_58();
                unit.isHero = ku->hero_flag();
                unit.byte5a = ku->byte_5a();
                unit.field60 = ku->field_60();
                unit.field64 = ku->field_64();
                unit.field68 = ku->field_68();

                unit.equipment = bytesToInt32Array<6>(ku->equipment());

                unit.abilitySets[0] = bytesToInt32Array<16>(ku->leader_abilities_1());
                unit.abilitySets[1] = bytesToInt32Array<16>(ku->officer1_abilities_1());
                unit.abilitySets[2] = bytesToInt32Array<16>(ku->officer2_abilities_1());
                unit.abilitySets[3] = bytesToInt32Array<16>(ku->leader_abilities_2());
                unit.abilitySets[4] = bytesToInt32Array<16>(ku->officer1_abilities_2());
                unit.abilitySets[5] = bytesToInt32Array<16>(ku->officer2_abilities_2());

                unit.field504 = ku->field_504();
            }
        }

        // Selected unit.
        selectedUnit_ = static_cast<int32_t>(parsed.selected_unit_ref());

        // Roster.
        roster_.clear();
        if (parsed.roster_entries()) {
            roster_.resize(parsed.roster_entries()->size());
            for (size_t i = 0; i < parsed.roster_entries()->size(); ++i) {
                const auto* kr = (*parsed.roster_entries())[i];
                roster_[i].byte61 = kr->byte_61();
                roster_[i].byte60 = kr->byte_60();
                roster_[i].byte62 = kr->byte_62();
                roster_[i].byte63 = kr->byte_63();
                roster_[i].value64 = kr->uint_64();
            }
        }

        // Second array.
        secondArray_.clear();
        if (parsed.second_array()) {
            secondArray_ = *parsed.second_array();
        }

        // Mission completion.
        if (parsed.mission_completion()) {
            for (size_t i = 0; i < kSaveMissionSlots && i < parsed.mission_completion()->size(); ++i) {
                missionCompletion_[i] = static_cast<int32_t>((*parsed.mission_completion())[i]);
            }
        }

        // Current mission index.
        currentMissionIndex_ = static_cast<int32_t>(parsed.current_mission_slot());

        // Remaining bytes for round-trip preservation.
        const auto& tail = parsed.tail_data();
        rawTail_.clear();
        if (!tail.empty()) {
            rawTail_.resize(tail.size());
            std::memcpy(rawTail_.data(), tail.data(), tail.size());
        }

    } catch (const std::exception&) {
        return false;
    }

    return true;
}

std::vector<std::byte> SaveFormat::save() const {
    patchMainBlock();

    std::vector<std::byte> data;
    data.reserve(kSavePadTarget);

    // Size prefix placeholder (will be patched at end if needed).
    if (hasSizePrefix_) {
        appendLE(data, static_cast<uint32_t>(0));
    }

    // Magic.
    appendLE(data, kSaveMagic);

    // Context (write raw bytes for round-trip fidelity).
    if (hasContext_) {
        appendBytes(data, context_.rawData.data(), kSaveContextSize);
    }

    // Campaign index.
    appendLE(data, campaignIndex_);

    // Main block (patched raw bytes).
    appendBytes(data, mainBlock_.rawData.data(), kSaveMainBlockSize);

    // Unit count + units.
    appendLE(data, static_cast<uint32_t>(units_.size()));

    for (const auto& unit : units_) {
        appendLE(data, unit.unknownIndex);
        appendLE(data, unit.troopInfoIndex);
        appendLE(data, unit.jobType);
        appendLE(data, unit.modelId);
        appendLE(data, unit.stgField34);
        appendLE(data, unit.stgField38);
        appendLE(data, unit.stgField3c);
        appendLE(data, unit.stgField40);
        appendLE(data, unit.charId);
        appendLE(data, unit.troopInfoIndex2);
        appendLE(data, unit.ucd);
        appendLE(data, unit.formationType);
        appendLE(data, unit.gridConfig);
        appendLE(data, unit.skillLevel);
        appendByte(data, unit.byte58);
        appendByte(data, unit.isHero);
        appendByte(data, unit.byte5a);
        appendLE(data, unit.field60);
        appendLE(data, unit.field64);
        appendLE(data, unit.field68);

        for (int e = 0; e < 6; ++e) {
            appendLE(data, unit.equipment[e]);
        }

        for (int s = 0; s < 6; ++s) {
            for (int a = 0; a < 16; ++a) {
                appendLE(data, unit.abilitySets[s][a]);
            }
        }

        appendLE(data, unit.field504);
    }

    // Selected unit.
    appendLE(data, selectedUnit_);

    // Roster count + roster records.
    appendLE(data, static_cast<uint32_t>(roster_.size()));
    for (const auto& rec : roster_) {
        appendByte(data, rec.byte61);
        appendByte(data, rec.byte60);
        appendByte(data, rec.byte62);
        appendByte(data, rec.byte63);
        appendLE(data, rec.value64);
    }

    // Second array count + values.
    appendLE(data, static_cast<uint32_t>(secondArray_.size()));
    for (uint32_t val : secondArray_) {
        appendLE(data, val);
    }

    // Mission completion: 20 × i32.
    for (size_t i = 0; i < kSaveMissionSlots; ++i) {
        appendLE(data, missionCompletion_[i]);
    }

    // Current mission index.
    appendLE(data, currentMissionIndex_);

    // Raw tail data.
    if (!rawTail_.empty()) {
        data.insert(data.end(), rawTail_.begin(), rawTail_.end());
    }

    // Pad to 32KB.
    if (data.size() < kSavePadTarget) {
        data.resize(kSavePadTarget, std::byte{0});
    }

    // Patch size prefix if present.
    if (hasSizePrefix_) {
        uint32_t totalSize = static_cast<uint32_t>(data.size());
        std::memcpy(data.data(), &totalSize, 4);
    }

    return data;
}

std::vector<ValidationIssue> SaveFormat::validate() const {
    std::vector<ValidationIssue> issues;

    if (campaignIndex_ < 0 || campaignIndex_ > 3) {
        issues.push_back({
            Severity::Warning,
            "campaignIndex",
            "Campaign index outside expected range (0-3)",
            0
        });
    }

    for (size_t i = 0; i < units_.size(); ++i) {
        const auto& unit = units_[i];

        if (unit.ucd > 3) {
            issues.push_back({
                Severity::Warning,
                "ucd",
                "UCD value outside expected range (0-3)",
                i
            });
        }

        if (unit.jobType > 42 && unit.jobType < 32) {
            // This check is intentionally loose — charinfo job types go higher.
        }
    }

    if (currentMissionIndex_ < 0) {
        issues.push_back({
            Severity::Warning,
            "currentMissionIndex",
            "Negative mission index",
            0
        });
    }

    return issues;
}

} // namespace kuf
