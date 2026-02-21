#include "formats/sox_skill_info.h"

#include "parsers/sox_skill_info.h"

#include <cstring>

namespace kuf {

bool SoxSkillInfo::load(std::span<const std::byte> data) {
	try {
		const auto *buf =
		    reinterpret_cast<const uint8_t *>(data.data());
		size_t offset = 0;
		auto file =
		    sox_skill_info::File::parse(buf, data.size(), offset);

		headerVersion_ = static_cast<int32_t>(file.header.marker);

		skills_.clear();
		skills_.reserve(file.records.size());
		for (const auto &rec : file.records) {
			SkillInfo skill{};
			skill.id = rec.skill_id;
			skill.locKey.assign(rec.loc_key.value.begin(),
					    rec.loc_key.value.end());
			skill.iconPath.assign(rec.icon.value.begin(),
					      rec.icon.value.end());
			skill.skillType = rec.skill_type;
			skill.maxLevel = rec.max_level;
			skills_.push_back(std::move(skill));
		}

		footer_.resize(64);
		std::memcpy(footer_.data(), file.footer, 64);
		version_ = GameVersion::Crusaders;
		return true;
	} catch (const std::exception &) {
		return false;
	}
}

std::vector<std::byte> SoxSkillInfo::save() const {
	sox_skill_info::File file{};
	file.header.marker = static_cast<uint32_t>(headerVersion_);
	file.header.record_count = static_cast<uint32_t>(skills_.size());

	for (const auto &skill : skills_) {
		sox_skill_info::SkillInfoRecord rec{};
		rec.skill_id = skill.id;
		rec.loc_key.length = static_cast<uint16_t>(skill.locKey.size());
		rec.loc_key.value.assign(skill.locKey.begin(),
					 skill.locKey.end());
		rec.icon.length = static_cast<uint16_t>(skill.iconPath.size());
		rec.icon.value.assign(skill.iconPath.begin(),
				      skill.iconPath.end());
		rec.skill_type = skill.skillType;
		rec.max_level = skill.maxLevel;
		file.records.push_back(std::move(rec));
	}

	std::memcpy(file.footer, footer_.data(),
		    std::min(footer_.size(), size_t{64}));

	auto bytes = file.to_bytes();
	std::vector<std::byte> result(bytes.size());
	std::memcpy(result.data(), bytes.data(), bytes.size());
	return result;
}

std::vector<ValidationIssue> SoxSkillInfo::validate() const {
	std::vector<ValidationIssue> issues;

	for (size_t i = 0; i < skills_.size(); ++i) {
		const auto &skill = skills_[i];

		if (skill.skillType != 1 && skill.skillType != 2) {
			issues.push_back(
			    {Severity::Warning, "skillType",
			     "Skill type should be 1 (Combat) or 2 (Magic)",
			     i});
		}

		if (skill.maxLevel == 0 || skill.maxLevel > 65535) {
			issues.push_back({Severity::Warning, "maxLevel",
					  "Max level is 0 or exceeds 65535",
					  i});
		}

		if (skill.locKey.empty()) {
			issues.push_back({Severity::Warning, "locKey",
					  "Localization key is empty", i});
		}

		if (skill.iconPath.empty()) {
			issues.push_back({Severity::Warning, "iconPath",
					  "Icon path is empty", i});
		}
	}

	return issues;
}

} // namespace kuf
