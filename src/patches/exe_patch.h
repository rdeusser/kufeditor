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

} // namespace kuf
