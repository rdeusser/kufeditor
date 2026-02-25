#pragma once

#include <cstdint>
#include <filesystem>
#include <vector>

namespace kuf {

struct BytePatch {
	uint32_t offset;
	std::vector<uint8_t> original;
	std::vector<uint8_t> patched;
};

struct ExePatch {
	const char *name;
	const char *description;
	std::vector<BytePatch> bytes;
};

enum class PatchStatus { Applied, NotApplied, Unknown };

std::vector<ExePatch> exePatches();
PatchStatus checkPatch(const std::filesystem::path &exe, const ExePatch &patch);
bool applyPatch(const std::filesystem::path &exe, const ExePatch &patch);
bool revertPatch(const std::filesystem::path &exe, const ExePatch &patch);

struct FireRateValues {
	int32_t baseDelay;
	int32_t multiplier;
	float distanceFactor;
};

struct FireRatePreset {
	const char *name;
	const char *description;
	FireRateValues values;
};

enum class FireRateStatus {
	Original,
	Fast,
	Rapid,
	Turbo,
	Custom,
	Unknown,
};

std::vector<FireRatePreset> fireRatePresets();
FireRateValues readFireRateValues(const std::filesystem::path &exe);
FireRateStatus checkFireRate(const std::filesystem::path &exe);
bool applyFireRate(const std::filesystem::path &exe,
		   const FireRateValues &values);

} // namespace kuf
