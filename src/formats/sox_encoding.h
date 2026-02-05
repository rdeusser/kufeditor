#pragma once

#include <cstddef>
#include <optional>
#include <span>
#include <vector>

namespace kuf {

// SOX files use ASCII hex encoding - each logical byte is stored as 2 ASCII hex characters.
// Example: uint32 value 100 (0x64) is stored as ASCII "64000000" (8 bytes in file).

// Decodes ASCII hex encoded SOX data to binary. Returns nullopt if the input is invalid.
std::optional<std::vector<std::byte>> soxDecode(std::span<const std::byte> encoded);

// Encodes binary data to ASCII hex for SOX format.
std::vector<std::byte> soxEncode(std::span<const std::byte> decoded);

// Checks if data appears to be ASCII hex encoded SOX data.
bool isSoxEncoded(std::span<const std::byte> data);

} // namespace kuf
