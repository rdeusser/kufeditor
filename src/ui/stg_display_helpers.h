#pragma once

#include "core/name_dictionary.h"
#include "formats/stg_format.h"

#include <cctype>
#include <cstring>
#include <string>
#include <vector>

namespace kuf {

// CharInfo job types that use CharInfo names (from GetUnitDisplayName at
// 0x005597a0).
inline constexpr uint8_t kCharInfoJobTypes[] = {32, 33, 34, 35, 36, 37,
						38, 43, 44, 46, 47};

inline bool isCharInfoJobType(uint8_t jobType) {
	for (uint8_t jt : kCharInfoJobTypes) {
		if (jt == jobType) return true;
	}
	return false;
}

inline bool asciiPrefixMatch(const std::vector<std::byte> &key,
			     const std::string &unitName) {
	if (key.empty() || unitName.size() < key.size()) return false;

	for (size_t i = 0; i < key.size(); ++i) {
		char a = static_cast<char>(key[i]);
		char b = unitName[i];
		if (std::tolower(static_cast<unsigned char>(a)) !=
		    std::tolower(static_cast<unsigned char>(b))) {
			return false;
		}
	}
	return true;
}

inline std::string resolveSpecialName(const std::string &unitName,
				      const NameDictionary &dict) {
	for (const auto &entry : dict.specialNames()) {
		if (asciiPrefixMatch(entry.keyBytes, unitName)) {
			return entry.displayName;
		}
	}
	return {};
}

inline std::string resolveDisplayName(const StgUnit &unit,
				      const NameDictionary &dict) {
	// Game-accurate priority chain from GetUnitDisplayName (0x005597a0).

	// 1. SpecialNames prefix match for:
	//    - Names starting with '-' (0x2D)
	//    - Paladin (job 6) with model > 12
	//    - DE Cav Archer (job 19) with model > 6
	bool trySpecial = false;
	if (!unit.unitName.empty() && unit.unitName[0] == '-') {
		trySpecial = true;
	} else if (unit.leaderJobType == 6 && unit.leaderModelId > 12) {
		trySpecial = true;
	} else if (unit.leaderJobType == 19 && unit.leaderModelId > 6) {
		trySpecial = true;
	}

	if (trySpecial) {
		std::string special = resolveSpecialName(unit.unitName, dict);
		if (!special.empty()) return special;
	}

	// 2. CharInfo name lookup for specific job types or DO Axe Man with
	// model < 1.
	if (unit.leaderJobType == 26 && unit.leaderModelId < 1) {
		const char *charName = dict.charInfoName(unit.leaderJobType);
		if (charName) return charName;
	} else if (isCharInfoJobType(unit.leaderJobType)) {
		const char *charName = dict.charInfoName(unit.leaderJobType);
		if (charName) return charName;
	}

	// 3. TroopInfo name for standard job types 0-42.
	if (unit.leaderJobType <= kMaxStandardJobType) {
		const char *troopName = dict.troopInfoName(unit.leaderJobType);
		if (troopName) return troopName;
	}

	// 4. Korean-to-English translation fallback.
	std::string translated = dict.translate(unit.unitName);
	if (!translated.empty()) return translated;

	// 5. Show the internal name rather than a meaningless "Unknown".
	if (!unit.unitName.empty()) return unit.unitName;

	return "Unknown";
}

inline bool isTroopIdHint(const char *hint) {
	if (!hint) return false;
	return std::strstr(hint, "TroopID") != nullptr ||
	       std::strcmp(hint, "TargetID") == 0 ||
	       std::strcmp(hint, "AttackerID") == 0;
}

} // namespace kuf
