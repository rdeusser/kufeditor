#pragma once

#include <cstddef>
#include <optional>
#include <span>
#include <vector>

namespace kuf {

// SOX files use pure binary encoding. Earlier community documentation claimed ASCII hex
// encoding but this was disproven by Ghidra decompilation. These functions remain as a
// fallback for any non-standard files that may use hex encoding.

// Decodes ASCII hex encoded SOX data to binary. Returns nullopt if the input is invalid.
std::optional<std::vector<std::byte>> soxDecode(std::span<const std::byte> encoded);

// Encodes binary data to ASCII hex for SOX format.
std::vector<std::byte> soxEncode(std::span<const std::byte> decoded);

// Checks if data appears to be ASCII hex encoded SOX data.
bool isSoxEncoded(std::span<const std::byte> data);

} // namespace kuf
