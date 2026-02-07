#include "formats/sox_encoding.h"

#include <cctype>
#include <cstdint>

namespace kuf {

namespace {

int hexCharToInt(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    return -1;
}

char intToHexChar(int v) {
    if (v < 10) return '0' + v;
    return 'A' + (v - 10);
}

} // namespace

std::optional<std::vector<std::byte>> soxDecode(std::span<const std::byte> encoded) {
    if (encoded.size() % 2 != 0) {
        return std::nullopt;
    }

    std::vector<std::byte> decoded;
    decoded.reserve(encoded.size() / 2);

    for (size_t i = 0; i < encoded.size(); i += 2) {
        int high = hexCharToInt(static_cast<char>(encoded[i]));
        int low = hexCharToInt(static_cast<char>(encoded[i + 1]));

        if (high < 0 || low < 0) {
            return std::nullopt;
        }

        decoded.push_back(static_cast<std::byte>((high << 4) | low));
    }

    return decoded;
}

std::vector<std::byte> soxEncode(std::span<const std::byte> decoded) {
    std::vector<std::byte> encoded;
    encoded.reserve(decoded.size() * 2);

    for (std::byte b : decoded) {
        int v = static_cast<int>(b);
        encoded.push_back(static_cast<std::byte>(intToHexChar((v >> 4) & 0xF)));
        encoded.push_back(static_cast<std::byte>(intToHexChar(v & 0xF)));
    }

    return encoded;
}

bool isSoxEncoded(std::span<const std::byte> data) {
    if (data.size() < 16) {
        return false;
    }

    // Check first 16 characters for ASCII hex validity.
    for (size_t i = 0; i < 16; ++i) {
        char c = static_cast<char>(data[i]);
        if (!std::isxdigit(static_cast<unsigned char>(c))) {
            return false;
        }
    }

    // Check if decoding the header gives marker 100 (0x64).
    int h0 = hexCharToInt(static_cast<char>(data[0]));
    int h1 = hexCharToInt(static_cast<char>(data[1]));
    int h2 = hexCharToInt(static_cast<char>(data[2]));
    int h3 = hexCharToInt(static_cast<char>(data[3]));

    if (h0 < 0 || h1 < 0 || h2 < 0 || h3 < 0) {
        return false;
    }

    // First uint32 little-endian should be 100 (0x64 0x00 0x00 0x00).
    uint8_t byte0 = (h0 << 4) | h1;
    uint8_t byte1 = (h2 << 4) | h3;

    return byte0 == 0x64 && byte1 == 0x00;
}

} // namespace kuf
