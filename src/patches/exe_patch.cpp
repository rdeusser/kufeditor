#include "patches/exe_patch.h"

#include <fstream>

namespace kuf {

namespace {

struct ContextCheck {
	uint32_t offset;
	std::vector<uint8_t> original;
	std::vector<uint8_t> patched;
};

// Context bytes surrounding each patch location for validation.
// Read 6 bytes starting 2 before the patch offset to verify this is the right
// exe.
std::vector<ContextCheck> debugMenuContext() {
	return {
	    {0x0D76EC,
	     {0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00},
	     {0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00}},
	    {0x0D7710,
	     {0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00},
	     {0x8B, 0x0D, 0xAC, 0x3C, 0x74, 0x00}},
	};
}

bool readBytes(const std::filesystem::path &path, uint32_t offset, uint8_t *buf,
	       size_t count) {
	std::ifstream file(path, std::ios::binary);
	if (!file) return false;
	file.seekg(offset);
	if (!file) return false;
	file.read(reinterpret_cast<char *>(buf),
		  static_cast<std::streamsize>(count));
	return file.good();
}

bool writeBytes(const std::filesystem::path &path, uint32_t offset,
		const uint8_t *buf, size_t count) {
	std::fstream file(path,
			  std::ios::binary | std::ios::in | std::ios::out);
	if (!file) return false;
	file.seekp(offset);
	if (!file) return false;
	file.write(reinterpret_cast<const char *>(buf),
		   static_cast<std::streamsize>(count));
	return file.good();
}

bool createBackup(const std::filesystem::path &exe) {
	auto backup = exe;
	backup += ".bak";
	if (std::filesystem::exists(backup)) return true;
	std::error_code ec;
	std::filesystem::copy_file(exe, backup, ec);
	return !ec;
}

bool validateContext(const std::filesystem::path &exe, const ExePatch &patch) {
	// Only the debug menu patch has context validation for now.
	// The terrain bounds patch validates via its 92 bytes of original data
	// in BytePatch.
	if (std::string(patch.name) != "Debug Menu") return true;

	auto checks = debugMenuContext();
	for (const auto &check : checks) {
		std::vector<uint8_t> buf(check.original.size());
		if (!readBytes(exe, check.offset, buf.data(), buf.size()))
			return false;
		if (buf != check.original && buf != check.patched) return false;
	}
	return true;
}

// Bounds-checking wrapper for the terrain height sampler (x86 machine code).
//
// On entry (called via CALL from FUN_0062e570):
//   ECX = terrain object (this)
//   [ESP+4] = X coordinate (float)
//   [ESP+8] = Z coordinate (float)
//
// Terrain object layout:
//   ECX+0x110 = grid_width (int32)
//   ECX+0x114 = grid_height (int32)
//
// Valid range: 0 < coord < grid_dim * 125.0
//
// If in bounds: tail-call JMP to FUN_00647b20
// If out of bounds: return 0.0 in FPU ST(0)
std::vector<uint8_t> terrainBoundsWrapper() {
	return {
	    // Check X > 0.0
	    0xF3,
	    0x0F,
	    0x10,
	    0x44,
	    0x24,
	    0x04, // movss xmm0, [esp+4]
	    0x0F,
	    0x57,
	    0xC9, // xorps xmm1, xmm1
	    0x0F,
	    0x2F,
	    0xC1, // comiss xmm0, xmm1
	    0x76,
	    0x46, // jbe out_of_bounds

	    // Check Z > 0.0
	    0xF3,
	    0x0F,
	    0x10,
	    0x44,
	    0x24,
	    0x08, // movss xmm0, [esp+8]
	    0x0F,
	    0x2F,
	    0xC1, // comiss xmm0, xmm1
	    0x76,
	    0x3B, // jbe out_of_bounds

	    // Check X < grid_width * 125.0
	    0xF3,
	    0x0F,
	    0x2A,
	    0x81,
	    0x10,
	    0x01,
	    0x00,
	    0x00, // cvtsi2ss xmm0, [ecx+0x110]
	    0xF3,
	    0x0F,
	    0x59,
	    0x05,
	    0x1C,
	    0xD5,
	    0x6B,
	    0x00, // mulss xmm0, [0x006BD51C] (125.0f)
	    0xF3,
	    0x0F,
	    0x10,
	    0x4C,
	    0x24,
	    0x04, // movss xmm1, [esp+4]
	    0x0F,
	    0x2F,
	    0xC1, // comiss xmm0, xmm1
	    0x76,
	    0x20, // jbe out_of_bounds

	    // Check Z < grid_height * 125.0
	    0xF3,
	    0x0F,
	    0x2A,
	    0x81,
	    0x14,
	    0x01,
	    0x00,
	    0x00, // cvtsi2ss xmm0, [ecx+0x114]
	    0xF3,
	    0x0F,
	    0x59,
	    0x05,
	    0x1C,
	    0xD5,
	    0x6B,
	    0x00, // mulss xmm0, [0x006BD51C] (125.0f)
	    0xF3,
	    0x0F,
	    0x10,
	    0x4C,
	    0x24,
	    0x08, // movss xmm1, [esp+8]
	    0x0F,
	    0x2F,
	    0xC1, // comiss xmm0, xmm1
	    0x76,
	    0x05, // jbe out_of_bounds

	    // In bounds: tail-call original terrain sampler
	    0xE9,
	    0xAE,
	    0xD9,
	    0xF8,
	    0xFF, // jmp FUN_00647b20

	    // Out of bounds: return 0.0
	    0xD9,
	    0xEE, // fldz
	    0xC3, // ret
	};
}

} // namespace

std::vector<ExePatch> exePatches() {
	return {
	    {
		"Debug Menu",
		"Redirect tilde key from PC Key/Mouse Settings to the "
		"developer "
		"debug menu (CTestMenu)",
		{
		    {0x0D76EE, {0xB0}, {0xAC}},
		    {0x0D7712, {0xB0}, {0xAC}},
		},
	    },
	    {
		"Terrain Bounds Check",
		"Fix crash from out-of-bounds terrain height queries (large "
		"SightRange or map edge positions)",
		{
		    {0x22D991,
		     {0xE8, 0x8A, 0x95, 0x01, 0x00},
		     {0xE8, 0x88, 0xBB, 0x08, 0x00}},
		    {0x2B951E, std::vector<uint8_t>(87, 0x00),
		     terrainBoundsWrapper()},
		},
	    },
	};
}

PatchStatus checkPatch(const std::filesystem::path &exe,
		       const ExePatch &patch) {
	bool allPatched = true;
	bool allOriginal = true;

	for (const auto &bp : patch.bytes) {
		std::vector<uint8_t> buf(bp.original.size());
		if (!readBytes(exe, bp.offset, buf.data(), buf.size()))
			return PatchStatus::Unknown;

		if (buf != bp.patched) allPatched = false;
		if (buf != bp.original) allOriginal = false;
	}

	if (allPatched) return PatchStatus::Applied;
	if (allOriginal) return PatchStatus::NotApplied;
	return PatchStatus::Unknown;
}

bool applyPatch(const std::filesystem::path &exe, const ExePatch &patch) {
	if (!validateContext(exe, patch)) return false;
	if (!createBackup(exe)) return false;

	for (const auto &bp : patch.bytes) {
		if (!writeBytes(exe, bp.offset, bp.patched.data(),
				bp.patched.size()))
			return false;
	}
	return true;
}

bool revertPatch(const std::filesystem::path &exe, const ExePatch &patch) {
	if (!validateContext(exe, patch)) return false;
	if (!createBackup(exe)) return false;

	for (const auto &bp : patch.bytes) {
		if (!writeBytes(exe, bp.offset, bp.original.data(),
				bp.original.size()))
			return false;
	}
	return true;
}

} // namespace kuf
