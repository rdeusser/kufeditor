#include <catch2/catch_test_macros.hpp>

#include "formats/sox_encoding.h"

#include <cstring>

TEST_CASE("soxDecode converts ASCII hex to bytes", "[sox_encoding]") {
    // ASCII "64000000" should decode to bytes 0x64 0x00 0x00 0x00 (little-endian 100).
    std::vector<std::byte> encoded = {
        std::byte{'6'}, std::byte{'4'}, std::byte{'0'}, std::byte{'0'},
        std::byte{'0'}, std::byte{'0'}, std::byte{'0'}, std::byte{'0'}
    };

    auto decoded = kuf::soxDecode(encoded);
    REQUIRE(decoded.has_value());
    REQUIRE(decoded->size() == 4);
    REQUIRE((*decoded)[0] == std::byte{0x64});
    REQUIRE((*decoded)[1] == std::byte{0x00});
    REQUIRE((*decoded)[2] == std::byte{0x00});
    REQUIRE((*decoded)[3] == std::byte{0x00});
}

TEST_CASE("soxDecode handles mixed case", "[sox_encoding]") {
    // "AbCdEf" should decode to 0xAB 0xCD 0xEF.
    std::vector<std::byte> encoded = {
        std::byte{'A'}, std::byte{'b'}, std::byte{'C'}, std::byte{'d'},
        std::byte{'E'}, std::byte{'f'}
    };

    auto decoded = kuf::soxDecode(encoded);
    REQUIRE(decoded.has_value());
    REQUIRE(decoded->size() == 3);
    REQUIRE((*decoded)[0] == std::byte{0xAB});
    REQUIRE((*decoded)[1] == std::byte{0xCD});
    REQUIRE((*decoded)[2] == std::byte{0xEF});
}

TEST_CASE("soxDecode rejects invalid input", "[sox_encoding]") {
    // Odd length is invalid.
    std::vector<std::byte> oddLen = {std::byte{'A'}};
    REQUIRE(!kuf::soxDecode(oddLen).has_value());

    // Non-hex characters are invalid.
    std::vector<std::byte> nonHex = {std::byte{'G'}, std::byte{'H'}};
    REQUIRE(!kuf::soxDecode(nonHex).has_value());
}

TEST_CASE("soxEncode converts bytes to ASCII hex", "[sox_encoding]") {
    std::vector<std::byte> binary = {
        std::byte{0x64}, std::byte{0x00}, std::byte{0xAB}, std::byte{0xFF}
    };

    auto encoded = kuf::soxEncode(binary);
    REQUIRE(encoded.size() == 8);
    REQUIRE(encoded[0] == std::byte{'6'});
    REQUIRE(encoded[1] == std::byte{'4'});
    REQUIRE(encoded[2] == std::byte{'0'});
    REQUIRE(encoded[3] == std::byte{'0'});
    REQUIRE(encoded[4] == std::byte{'A'});
    REQUIRE(encoded[5] == std::byte{'B'});
    REQUIRE(encoded[6] == std::byte{'F'});
    REQUIRE(encoded[7] == std::byte{'F'});
}

TEST_CASE("soxEncode/soxDecode round-trip", "[sox_encoding]") {
    std::vector<std::byte> original = {
        std::byte{0x64}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00},
        std::byte{0x2B}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00}
    };

    auto encoded = kuf::soxEncode(original);
    auto decoded = kuf::soxDecode(encoded);

    REQUIRE(decoded.has_value());
    REQUIRE(decoded->size() == original.size());
    REQUIRE(std::memcmp(decoded->data(), original.data(), original.size()) == 0);
}

TEST_CASE("isSoxEncoded detects hex-encoded SOX header", "[sox_encoding]") {
    // Valid SOX header: "64000000" (version 100 in little-endian hex).
    std::vector<std::byte> valid(32, std::byte{'0'});
    valid[0] = std::byte{'6'};
    valid[1] = std::byte{'4'};

    REQUIRE(kuf::isSoxEncoded(valid));

    // Not hex encoded: raw binary.
    std::vector<std::byte> binary = {
        std::byte{0x64}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00},
        std::byte{0x01}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00},
        std::byte{0x00}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00},
        std::byte{0x00}, std::byte{0x00}, std::byte{0x00}, std::byte{0x00}
    };
    REQUIRE(!kuf::isSoxEncoded(binary));
}
