#include "formats/sox_binary.h"

#include "parsers/sox_troop_info.h"

#include <cstring>

namespace kuf {

namespace {

TroopInfo wireToTroop(const sox_troop_info::TroopInfoRecord &r) {
	TroopInfo t{};
	t.job = r.job;
	t.typeId = r.type_id;
	t.moveSpeed = static_cast<float>(r.move_speed);
	t.rotateRate = static_cast<float>(r.rotate_rate);
	t.moveAcceleration = static_cast<float>(r.move_acceleration);
	t.moveDeceleration = static_cast<float>(r.move_deceleration);
	t.sightRange = static_cast<float>(r.sight_range);
	t.attackRangeMax = static_cast<float>(r.attack_range_max);
	t.attackRangeMin = static_cast<float>(r.attack_range_min);
	t.attackFrontRange = static_cast<float>(r.attack_front_range);
	t.directAttack = static_cast<float>(r.direct_attack);
	t.indirectAttack = static_cast<float>(r.indirect_attack);
	t.defense = static_cast<float>(r.defense);
	t.baseWidth = static_cast<float>(r.base_width);
	t.resistMelee = static_cast<float>(r.resist_melee);
	t.resistRanged = static_cast<float>(r.resist_ranged);
	t.resistFrontal = static_cast<float>(r.resist_frontal);
	t.resistExplosion = static_cast<float>(r.resist_explosion);
	t.resistFire = static_cast<float>(r.resist_fire);
	t.resistIce = static_cast<float>(r.resist_ice);
	t.resistLightning = static_cast<float>(r.resist_lightning);
	t.resistHoly = static_cast<float>(r.resist_holy);
	t.resistCurse = static_cast<float>(r.resist_curse);
	t.resistEarth = static_cast<float>(r.resist_earth);
	t.maxUnitSpeedMultiplier =
	    static_cast<float>(r.max_unit_speed_multiplier);
	t.defaultUnitHp = static_cast<float>(r.default_unit_hp);
	t.formationRandom = r.formation_random;
	t.defaultUnitNumX = r.default_unit_num_x;
	t.defaultUnitNumY = r.default_unit_num_y;
	t.unitHpLevelUp = static_cast<float>(r.unit_hp_level_up);
	t.levelUpData[0] = {r.level_up_0_skill_id,
			    static_cast<float>(r.level_up_0_bonus)};
	t.levelUpData[1] = {r.level_up_1_skill_id,
			    static_cast<float>(r.level_up_1_bonus)};
	t.levelUpData[2] = {r.level_up_2_skill_id,
			    static_cast<float>(r.level_up_2_bonus)};
	t.damageDistribution = static_cast<float>(r.damage_distribution);
	return t;
}

sox_troop_info::TroopInfoRecord troopToWire(const TroopInfo &t) {
	sox_troop_info::TroopInfoRecord r{};
	r.job = t.job;
	r.type_id = t.typeId;
	r.move_speed = static_cast<int32_t>(t.moveSpeed);
	r.rotate_rate = static_cast<int32_t>(t.rotateRate);
	r.move_acceleration = static_cast<int32_t>(t.moveAcceleration);
	r.move_deceleration = static_cast<int32_t>(t.moveDeceleration);
	r.sight_range = static_cast<int32_t>(t.sightRange);
	r.attack_range_max = static_cast<int32_t>(t.attackRangeMax);
	r.attack_range_min = static_cast<int32_t>(t.attackRangeMin);
	r.attack_front_range = static_cast<int32_t>(t.attackFrontRange);
	r.direct_attack = static_cast<int32_t>(t.directAttack);
	r.indirect_attack = static_cast<int32_t>(t.indirectAttack);
	r.defense = static_cast<int32_t>(t.defense);
	r.base_width = static_cast<int32_t>(t.baseWidth);
	r.resist_melee = static_cast<int32_t>(t.resistMelee);
	r.resist_ranged = static_cast<int32_t>(t.resistRanged);
	r.resist_frontal = static_cast<int32_t>(t.resistFrontal);
	r.resist_explosion = static_cast<int32_t>(t.resistExplosion);
	r.resist_fire = static_cast<int32_t>(t.resistFire);
	r.resist_ice = static_cast<int32_t>(t.resistIce);
	r.resist_lightning = static_cast<int32_t>(t.resistLightning);
	r.resist_holy = static_cast<int32_t>(t.resistHoly);
	r.resist_curse = static_cast<int32_t>(t.resistCurse);
	r.resist_earth = static_cast<int32_t>(t.resistEarth);
	r.max_unit_speed_multiplier =
	    static_cast<int32_t>(t.maxUnitSpeedMultiplier);
	r.default_unit_hp = static_cast<int32_t>(t.defaultUnitHp);
	r.formation_random = t.formationRandom;
	r.default_unit_num_x = t.defaultUnitNumX;
	r.default_unit_num_y = t.defaultUnitNumY;
	r.unit_hp_level_up = static_cast<int32_t>(t.unitHpLevelUp);
	r.level_up_0_skill_id = t.levelUpData[0].skillId;
	r.level_up_0_bonus =
	    static_cast<int32_t>(t.levelUpData[0].bonusPerLevel);
	r.level_up_1_skill_id = t.levelUpData[1].skillId;
	r.level_up_1_bonus =
	    static_cast<int32_t>(t.levelUpData[1].bonusPerLevel);
	r.level_up_2_skill_id = t.levelUpData[2].skillId;
	r.level_up_2_bonus =
	    static_cast<int32_t>(t.levelUpData[2].bonusPerLevel);
	r.damage_distribution = static_cast<int32_t>(t.damageDistribution);
	return r;
}

} // namespace

bool SoxBinary::load(std::span<const std::byte> data) {
	try {
		const auto *buf =
		    reinterpret_cast<const uint8_t *>(data.data());
		size_t offset = 0;
		auto file =
		    sox_troop_info::File::parse(buf, data.size(), offset);

		headerVersion_ = static_cast<int32_t>(file.header.marker);

		troops_.clear();
		troops_.reserve(file.records.size());
		for (const auto &rec : file.records) {
			troops_.push_back(wireToTroop(rec));
		}

		footer_.resize(64);
		std::memcpy(footer_.data(), file.footer, 64);
		version_ = GameVersion::Crusaders;
		return true;
	} catch (const std::exception &) {
		return false;
	}
}

std::vector<std::byte> SoxBinary::save() const {
	sox_troop_info::File file{};
	file.header.marker = static_cast<uint32_t>(headerVersion_);
	file.header.record_count = static_cast<uint32_t>(troops_.size());

	for (const auto &troop : troops_) {
		file.records.push_back(troopToWire(troop));
	}

	std::memcpy(file.footer, footer_.data(),
		    std::min(footer_.size(), size_t{64}));

	auto bytes = file.to_bytes();
	std::vector<std::byte> result(bytes.size());
	std::memcpy(result.data(), bytes.data(), bytes.size());
	return result;
}

std::vector<ValidationIssue> SoxBinary::validate() const {
	std::vector<ValidationIssue> issues;

	for (size_t i = 0; i < troops_.size(); ++i) {
		const auto &troop = troops_[i];

		// Resistances: 0=immune, 100=normal, 250+=very vulnerable,
		// 1000000+=instant death. Only flag negative values or
		// extremely high non-instant-death values.
		auto checkResistance = [&](float value, const char *name) {
			int v = static_cast<int>(value);
			if (v < 0 || (v > 500 && v < 1000000)) {
				issues.push_back(
				    {Severity::Warning, name,
				     "Resistance outside typical range", i});
			}
		};

		checkResistance(troop.resistMelee, "resistMelee");
		checkResistance(troop.resistRanged, "resistRanged");
		checkResistance(troop.resistFrontal, "resistFrontal");
		checkResistance(troop.resistExplosion, "resistExplosion");
		checkResistance(troop.resistFire, "resistFire");
		checkResistance(troop.resistIce, "resistIce");
		checkResistance(troop.resistLightning, "resistLightning");
		checkResistance(troop.resistHoly, "resistHoly");
		checkResistance(troop.resistCurse, "resistCurse");
		checkResistance(troop.resistEarth, "resistEarth");

		if (troop.defaultUnitHp <= 0) {
			issues.push_back({Severity::Error, "defaultUnitHp",
					  "HP must be positive", i});
		}
	}

	return issues;
}

} // namespace kuf
