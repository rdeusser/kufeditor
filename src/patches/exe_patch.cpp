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
// Read 6 bytes starting 2 before the patch offset to verify this is the right exe.
std::vector<ContextCheck> debugMenuContext() {
    return {
        {0x0D76EC, {0x8B, 0x35, 0xB0, 0x3C, 0x74, 0x00}, {0x8B, 0x35, 0xAC, 0x3C, 0x74, 0x00}},
        {0x0D7710, {0x8B, 0x0D, 0xB0, 0x3C, 0x74, 0x00}, {0x8B, 0x0D, 0xAC, 0x3C, 0x74, 0x00}},
    };
}

bool readBytes(const std::filesystem::path& path, uint32_t offset, uint8_t* buf, size_t count) {
    std::ifstream file(path, std::ios::binary);
    if (!file) return false;
    file.seekg(offset);
    if (!file) return false;
    file.read(reinterpret_cast<char*>(buf), static_cast<std::streamsize>(count));
    return file.good();
}

bool writeBytes(const std::filesystem::path& path, uint32_t offset, const uint8_t* buf, size_t count) {
    std::fstream file(path, std::ios::binary | std::ios::in | std::ios::out);
    if (!file) return false;
    file.seekp(offset);
    if (!file) return false;
    file.write(reinterpret_cast<const char*>(buf), static_cast<std::streamsize>(count));
    return file.good();
}

bool createBackup(const std::filesystem::path& exe) {
    auto backup = exe;
    backup += ".bak";
    if (std::filesystem::exists(backup)) return true;
    std::error_code ec;
    std::filesystem::copy_file(exe, backup, ec);
    return !ec;
}

bool validateContext(const std::filesystem::path& exe, const ExePatch& patch) {
    // Only the debug menu patch has context validation for now.
    if (std::string(patch.name) != "Debug Menu") return true;

    auto checks = debugMenuContext();
    for (const auto& check : checks) {
        std::vector<uint8_t> buf(check.original.size());
        if (!readBytes(exe, check.offset, buf.data(), buf.size())) return false;
        if (buf != check.original && buf != check.patched) return false;
    }
    return true;
}

} // namespace

std::vector<ExePatch> exePatches() {
    return {
        {
            "Debug Menu",
            "Redirect tilde key from PC Key/Mouse Settings to the developer debug menu (CTestMenu)",
            {
                {0x0D76EE, 0xB0, 0xAC},
                {0x0D7712, 0xB0, 0xAC},
            },
        },
    };
}

PatchStatus checkPatch(const std::filesystem::path& exe, const ExePatch& patch) {
    bool allPatched = true;
    bool allOriginal = true;

    for (const auto& bp : patch.bytes) {
        uint8_t byte = 0;
        if (!readBytes(exe, bp.offset, &byte, 1)) return PatchStatus::Unknown;

        if (byte != bp.patched) allPatched = false;
        if (byte != bp.original) allOriginal = false;
    }

    if (allPatched) return PatchStatus::Applied;
    if (allOriginal) return PatchStatus::NotApplied;
    return PatchStatus::Unknown;
}

bool applyPatch(const std::filesystem::path& exe, const ExePatch& patch) {
    if (!validateContext(exe, patch)) return false;
    if (!createBackup(exe)) return false;

    for (const auto& bp : patch.bytes) {
        if (!writeBytes(exe, bp.offset, &bp.patched, 1)) return false;
    }
    return true;
}

bool revertPatch(const std::filesystem::path& exe, const ExePatch& patch) {
    if (!validateContext(exe, patch)) return false;
    if (!createBackup(exe)) return false;

    for (const auto& bp : patch.bytes) {
        if (!writeBytes(exe, bp.offset, &bp.original, 1)) return false;
    }
    return true;
}

} // namespace kuf
