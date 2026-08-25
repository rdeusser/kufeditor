#include <catch2/catch_test_macros.hpp>

#include "formats/save_format.h"

#include <algorithm>
#include <array>
#include <cstring>
#include <vector>

namespace {

void appendU32(std::vector<std::byte> &v, uint32_t val) {
	size_t pos = v.size();
	v.resize(pos + 4);
	std::memcpy(v.data() + pos, &val, 4);
}

void appendI32(std::vector<std::byte> &v, int32_t val) {
	size_t pos = v.size();
	v.resize(pos + 4);
	std::memcpy(v.data() + pos, &val, 4);
}

void appendU8(std::vector<std::byte> &v, uint8_t val) {
	v.push_back(static_cast<std::byte>(val));
}

void appendU16(std::vector<std::byte> &v, uint16_t val) {
	size_t pos = v.size();
	v.resize(pos + 2);
	std::memcpy(v.data() + pos, &val, 2);
}

void appendI16(std::vector<std::byte> &v, int16_t val) {
	size_t pos = v.size();
	v.resize(pos + 2);
	std::memcpy(v.data() + pos, &val, 2);
}

void appendZeros(std::vector<std::byte> &v, size_t count) {
	v.resize(v.size() + count, std::byte{0});
}

// Write an empty EquipmentSlot (64 bytes) with item_type_id = -1.
void appendEmptyEquipmentSlot(std::vector<std::byte> &v) {
	appendU32(v, 0);  // auto_id
	appendI32(v, -1); // item_type_id
	appendU16(v, 0);  // level
	appendI16(v, -1); // enhancement_tier
	appendU16(v, 0);  // variant_index
	appendI16(v, 0);  // item_power
	appendU16(v, 0);  // equipped_flag
	appendU16(v, 0);  // reserved
	appendI32(v, -1); // attribute1_index
	appendI32(v, -1); // attribute2_index
	appendI32(v, -1); // skill_type_1
	appendI32(v, 0);  // skill_bonus_1
	appendI32(v, -1); // skill_type_2
	appendI32(v, 0);  // skill_bonus_2
	appendI32(v, -1); // resist_type_1
	appendI32(v, 0);  // resist_bonus_1
	appendI32(v, -1); // resist_type_2
	appendI32(v, 0);  // resist_bonus_2
	appendI32(v, 0);  // slot_category
}

// Build a unit save data block (483 bytes).
void appendUnit(std::vector<std::byte> &v, int32_t troopIdx, uint32_t jobType,
		int32_t charId, uint32_t ucd, uint32_t skillLevel,
		uint8_t isHero) {
	size_t start = v.size();

	appendI32(v, 0);	  // leader_name_index
	appendI32(v, troopIdx);	  // troop_info_index
	appendU32(v, jobType);	  // job_type
	appendU32(v, 0);	  // model_id
	appendU32(v, 0);	  // stg_field_34
	appendU32(v, 0);	  // stg_field_38
	appendU32(v, 0);	  // stg_field_3c
	appendU32(v, 0);	  // stg_field_40
	appendI32(v, charId);	  // char_id
	appendI32(v, troopIdx);	  // troop_info_index_2
	appendU32(v, ucd);	  // ucd
	appendU32(v, 0);	  // formation_type
	appendU32(v, 0);	  // grid_config
	appendU32(v, skillLevel); // skill_level
	appendU8(v, 0);		  // byte_58
	appendU8(v, isHero);	  // is_hero
	appendU8(v, 0);		  // byte_5a
	appendU32(v, 0);	  // field_60
	appendU32(v, 0);	  // field_64
	appendU32(v, 0);	  // field_68

	// Equipment raw: 24 zero bytes.
	appendZeros(v, 24);

	// 6 × EquipmentSlot (64 bytes each = 384 bytes).
	for (int i = 0; i < 6; ++i) {
		appendEmptyEquipmentSlot(v);
	}

	// field_504.
	appendU32(v, 0);

	size_t written = v.size() - start;
	REQUIRE(written == kuf::kSaveUnitSize);
}

// Build a canonical save file (with size prefix and context block).
std::vector<std::byte> createMinimalSave(int32_t campaignIndex = 0,
					 int unitCount = 1) {
	std::vector<std::byte> data;

	// Size prefix placeholder (patched at end).
	appendU32(data, 0);

	// Magic.
	appendU32(data, kuf::kSaveMagic);

	// Context block: 0x438 bytes. First u32 = 0xFFFFFFFF for heuristic
	// detection.
	appendU32(data, 0xFFFFFFFF);
	appendZeros(data, kuf::kSaveContextSize - 4);

	// Campaign index.
	appendI32(data, campaignIndex);

	// Main block: 0x154 bytes of zeros.
	appendZeros(data, kuf::kSaveMainBlockSize);

	// Unit count + units.
	appendU32(data, static_cast<uint32_t>(unitCount));
	for (int i = 0; i < unitCount; ++i) {
		appendUnit(data, i, 3, i, 0, 5, 0);
	}

	// Selected unit.
	appendI32(data, 0);

	// Roster: count = 0.
	appendU32(data, 0);

	// Second array: count = 0.
	appendU32(data, 0);

	// Mission completion: 20 × i32.
	for (int i = 0; i < 20; ++i) {
		appendI32(data, 0);
	}

	// Current mission index.
	appendI32(data, 0);

	// Pad to 32KB.
	if (data.size() < kuf::kSavePadTarget) {
		data.resize(kuf::kSavePadTarget, std::byte{0});
	}

	// Patch size prefix.
	uint32_t totalSize = static_cast<uint32_t>(data.size());
	std::memcpy(data.data(), &totalSize, 4);

	return data;
}

} // namespace

TEST_CASE("SaveFormat rejects bad magic", "[save]") {
	kuf::SaveFormat save;
	std::vector<std::byte> data(100, std::byte{0});
	REQUIRE_FALSE(save.load(data));
}

TEST_CASE("SaveFormat rejects too-small data", "[save]") {
	kuf::SaveFormat save;
	std::vector<std::byte> data(4, std::byte{0});
	REQUIRE_FALSE(save.load(data));
}

TEST_CASE("SaveFormat parses canonical save", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave(2, 3);

	REQUIRE(save.load(data));
	REQUIRE(save.hasSizePrefix());
	REQUIRE(save.hasContext());
	REQUIRE(save.campaignIndex() == 2);
	REQUIRE(save.unitCount() == 3);
	REQUIRE(save.selectedUnit() == 0);
	REQUIRE(save.roster().empty());
	REQUIRE(save.secondArray().empty());
	REQUIRE(save.currentMissionIndex() == 0);
}

TEST_CASE("SaveFormat parses unit fields correctly", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave(0, 1);

	REQUIRE(save.load(data));
	REQUIRE(save.unitCount() == 1);

	const auto &unit = save.units()[0];
	REQUIRE(unit.troopInfoIndex == 0);
	REQUIRE(unit.jobType == 3);
	REQUIRE(unit.charId == 0);
	REQUIRE(unit.ucd == 0);
	REQUIRE(unit.skillLevel == 5);
	REQUIRE(unit.isHero == 0);

	// All equipment slots should be empty.
	for (int i = 0; i < 6; ++i) {
		REQUIRE(unit.equipSlots()[i].empty());
		REQUIRE(unit.equipSlots()[i].itemTypeId == -1);
	}
}

TEST_CASE("SaveFormat round-trip preserves data", "[save]") {
	kuf::SaveFormat save;
	auto original = createMinimalSave(1, 2);

	REQUIRE(save.load(original));
	auto saved = save.save();

	REQUIRE(saved.size() == original.size());
	REQUIRE(std::memcmp(saved.data(), original.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat modified fields survive round-trip", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave(0, 1);

	REQUIRE(save.load(data));

	save.units()[0].skillLevel = 99;
	save.units()[0].jobType = 6;
	save.setCampaignIndex(3);
	save.setCurrentMissionIndex(5);

	auto saved = save.save();

	kuf::SaveFormat save2;
	REQUIRE(save2.load(saved));

	REQUIRE(save2.campaignIndex() == 3);
	REQUIRE(save2.currentMissionIndex() == 5);
	REQUIRE(save2.units()[0].skillLevel == 99);
	REQUIRE(save2.units()[0].jobType == 6);
}

TEST_CASE("SaveFormat preserves unknown raw discriminants across edits",
	  "[save]") {
	auto data = createMinimalSave(0, 1);

	constexpr size_t unitOffset = 4 + 4 + kuf::kSaveContextSize + 4 +
				      kuf::kSaveMainBlockSize + 4;
	constexpr size_t UCDOffset = unitOffset + 40;
	constexpr size_t leaderWeaponOffset = unitOffset + 95;
	constexpr size_t skillType1Offset = leaderWeaponOffset + 28;
	constexpr size_t resistType1Offset = leaderWeaponOffset + 44;
	constexpr std::array unknownUCDBytes = {
	    std::byte{0x63}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00}};
	constexpr std::array unknownSkillTypeBytes = {
	    std::byte{0xD2}, std::byte{0x04}, std::byte{0x00}, std::byte{0x00}};
	constexpr std::array unknownResistTypeBytes = {
	    std::byte{0x9D}, std::byte{0xFF}, std::byte{0xFF}, std::byte{0xFF}};
	std::copy(unknownUCDBytes.begin(), unknownUCDBytes.end(),
		  data.begin() + UCDOffset);
	std::copy(unknownSkillTypeBytes.begin(), unknownSkillTypeBytes.end(),
		  data.begin() + skillType1Offset);
	std::copy(unknownResistTypeBytes.begin(), unknownResistTypeBytes.end(),
		  data.begin() + resistType1Offset);

	kuf::SaveFormat save;
	REQUIRE(save.load(data));
	save.units()[0].skillLevel = 77;

	auto saved = save.save();
	REQUIRE(std::equal(unknownUCDBytes.begin(), unknownUCDBytes.end(),
			   saved.begin() + UCDOffset));
	REQUIRE(std::equal(unknownSkillTypeBytes.begin(),
			   unknownSkillTypeBytes.end(),
			   saved.begin() + skillType1Offset));
	REQUIRE(std::equal(unknownResistTypeBytes.begin(),
			   unknownResistTypeBytes.end(),
			   saved.begin() + resistType1Offset));

	kuf::SaveFormat reloaded;
	REQUIRE(reloaded.load(saved));

	const auto &unit = reloaded.units()[0];
	REQUIRE(unit.ucd == 99);
	REQUIRE(unit.primaryWeapon.skillType1 == 1234);
	REQUIRE(unit.primaryWeapon.resistType1 == -99);
	REQUIRE(unit.skillLevel == 77);
}

TEST_CASE("SaveFormat pads to 32KB", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave();

	REQUIRE(save.load(data));
	auto saved = save.save();

	REQUIRE(saved.size() == kuf::kSavePadTarget);
}

TEST_CASE("SaveFormat handles data without size prefix", "[save]") {
	// Build data without size prefix: magic + context + rest.
	std::vector<std::byte> data;

	appendU32(data, kuf::kSaveMagic);

	// Context block.
	appendU32(data, 0xFFFFFFFF);
	appendZeros(data, kuf::kSaveContextSize - 4);

	appendI32(data, 0); // campaign
	appendZeros(data, kuf::kSaveMainBlockSize);
	appendU32(data, 1); // 1 unit
	appendUnit(data, 0, 3, 0, 0, 5, 0);
	appendI32(data, 0); // selected
	appendU32(data, 0); // roster
	appendU32(data, 0); // second array
	for (int i = 0; i < 20; ++i)
		appendI32(data, 0);
	appendI32(data, 0);

	if (data.size() < kuf::kSavePadTarget) {
		data.resize(kuf::kSavePadTarget, std::byte{0});
	}

	kuf::SaveFormat save;
	REQUIRE(save.load(data));
	REQUIRE_FALSE(save.hasSizePrefix());
	REQUIRE(save.hasContext());
	REQUIRE(save.unitCount() == 1);

	auto saved = save.save();
	REQUIRE(saved.size() == data.size());
	REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat handles data without context block", "[save]") {
	// Build data with prefix but no context: prefix + magic + campaign(0) +
	// rest.
	std::vector<std::byte> data;

	appendU32(data, 0); // prefix placeholder

	appendU32(data, kuf::kSaveMagic);

	// No context — campaign index directly after magic.
	appendI32(data, 1);

	appendZeros(data, kuf::kSaveMainBlockSize);
	appendU32(data, 0);  // 0 units
	appendI32(data, -1); // selected
	appendU32(data, 0);  // roster
	appendU32(data, 0);  // second array
	for (int i = 0; i < 20; ++i)
		appendI32(data, 0);
	appendI32(data, 0);

	if (data.size() < kuf::kSavePadTarget) {
		data.resize(kuf::kSavePadTarget, std::byte{0});
	}

	// Patch size prefix.
	uint32_t totalSize = static_cast<uint32_t>(data.size());
	std::memcpy(data.data(), &totalSize, 4);

	kuf::SaveFormat save;
	REQUIRE(save.load(data));
	REQUIRE(save.hasSizePrefix());
	REQUIRE_FALSE(save.hasContext());
	REQUIRE(save.campaignIndex() == 1);

	auto saved = save.save();
	REQUIRE(saved.size() == data.size());
	REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat preserves raw tail", "[save]") {
	std::vector<std::byte> data;

	// Size prefix placeholder.
	appendU32(data, 0);

	// Magic.
	appendU32(data, kuf::kSaveMagic);

	// Context block.
	appendU32(data, 0xFFFFFFFF);
	appendZeros(data, kuf::kSaveContextSize - 4);

	// Campaign index.
	appendI32(data, 0);

	// Main block.
	appendZeros(data, kuf::kSaveMainBlockSize);

	// 0 units.
	appendU32(data, 0);

	// Selected unit.
	appendI32(data, -1);

	// Roster: 0.
	appendU32(data, 0);

	// Second array: 0.
	appendU32(data, 0);

	// Mission completion.
	for (int i = 0; i < 20; ++i) {
		appendI32(data, 0);
	}

	// Current mission index.
	appendI32(data, 0);

	// Tail data: some non-zero bytes that should be preserved.
	for (int i = 0; i < 100; ++i) {
		data.push_back(static_cast<std::byte>(0xAB));
	}

	// Pad to 32KB.
	if (data.size() < kuf::kSavePadTarget) {
		data.resize(kuf::kSavePadTarget, std::byte{0});
	}

	// Patch size prefix.
	uint32_t totalSize = static_cast<uint32_t>(data.size());
	std::memcpy(data.data(), &totalSize, 4);

	kuf::SaveFormat save;
	REQUIRE(save.load(data));

	auto saved = save.save();
	REQUIRE(saved.size() == data.size());
	REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat roster round-trip", "[save]") {
	std::vector<std::byte> data;

	appendU32(data, 0); // prefix placeholder
	appendU32(data, kuf::kSaveMagic);

	// Context block.
	appendU32(data, 0xFFFFFFFF);
	appendZeros(data, kuf::kSaveContextSize - 4);

	appendI32(data, 0);
	appendZeros(data, kuf::kSaveMainBlockSize);

	// 0 units.
	appendU32(data, 0);
	appendI32(data, -1);

	// Roster: 2 entries.
	appendU32(data, 2);
	// Entry 0: byte_61=10, byte_60=20, byte_62=30, byte_63=40,
	// value_64=100.
	appendU8(data, 10);
	appendU8(data, 20);
	appendU8(data, 30);
	appendU8(data, 40);
	appendU32(data, 100);
	// Entry 1.
	appendU8(data, 1);
	appendU8(data, 2);
	appendU8(data, 3);
	appendU8(data, 4);
	appendU32(data, 200);

	appendU32(data, 0); // second array
	for (int i = 0; i < 20; ++i)
		appendI32(data, 0);
	appendI32(data, 0);

	if (data.size() < kuf::kSavePadTarget) {
		data.resize(kuf::kSavePadTarget, std::byte{0});
	}

	// Patch size prefix.
	uint32_t totalSize = static_cast<uint32_t>(data.size());
	std::memcpy(data.data(), &totalSize, 4);

	kuf::SaveFormat save;
	REQUIRE(save.load(data));
	REQUIRE(save.roster().size() == 2);
	REQUIRE(save.roster()[0].byte61 == 10);
	REQUIRE(save.roster()[0].byte60 == 20);
	REQUIRE(save.roster()[0].byte62 == 30);
	REQUIRE(save.roster()[0].byte63 == 40);
	REQUIRE(save.roster()[0].value64 == 100);
	REQUIRE(save.roster()[1].value64 == 200);

	auto saved = save.save();
	REQUIRE(saved.size() == data.size());
	REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat mission completion round-trip", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave();

	// Write some mission completion values.
	// Offset: prefix(4) + magic(4) + context(0x438) + campaign(4) +
	// mainblock(0x154) + unitcount(4) + 1 unit(483) + selected(4) +
	// roster_count(4) + second_count(4) = 4 + 4 + 1080 + 4 + 340 + 4 + 483
	// + 4 + 4 + 4 = 1931 Then 20 × i32 mission values start at offset 1931.
	constexpr size_t missionOffset = 4 + 4 + kuf::kSaveContextSize + 4 +
					 kuf::kSaveMainBlockSize + 4 +
					 kuf::kSaveUnitSize + 4 + 4 + 4;
	int32_t val5 = 5;
	int32_t val10 = 10;
	std::memcpy(data.data() + missionOffset, &val5, 4);	      // slot 0
	std::memcpy(data.data() + missionOffset + 19 * 4, &val10, 4); // slot 19

	// Re-patch size prefix since data was modified (values unchanged in
	// size).
	uint32_t totalSize = static_cast<uint32_t>(data.size());
	std::memcpy(data.data(), &totalSize, 4);

	REQUIRE(save.load(data));
	REQUIRE(save.missionCompletion()[0] == 5);
	REQUIRE(save.missionCompletion()[19] == 10);

	auto saved = save.save();
	REQUIRE(saved.size() == data.size());
	REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SaveFormat validate catches bad UCD", "[save]") {
	kuf::SaveFormat save;
	auto data = createMinimalSave(0, 1);

	REQUIRE(save.load(data));

	// Set UCD to invalid value.
	save.units()[0].ucd = 99;

	auto issues = save.validate();
	bool foundUcd = false;
	for (const auto &issue : issues) {
		if (issue.field == "ucd") {
			foundUcd = true;
			break;
		}
	}
	REQUIRE(foundUcd);
}
