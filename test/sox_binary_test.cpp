#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_floating_point.hpp>

#include "formats/sox_binary.h"

#include <array>
#include <cstring>

namespace {

std::vector<std::byte> createMinimalTroopInfoSox() {
    std::vector<std::byte> data(8 + 148 + 64, std::byte{0});

    // Header: version=100, count=1.
    int32_t version = 100;
    int32_t count = 1;
    std::memcpy(data.data(), &version, 4);
    std::memcpy(data.data() + 4, &count, 4);

    // First troop: set some recognizable values (file stores integers, not floats).
    int32_t moveSpeed = 130;
    int32_t resistMelee = 100;  // 100 = normal damage
    int32_t defaultHp = 800;
    std::memcpy(data.data() + 8 + 0x08, &moveSpeed, 4);
    std::memcpy(data.data() + 8 + 0x38, &resistMelee, 4);
    std::memcpy(data.data() + 8 + 0x64, &defaultHp, 4);

    return data;
}

} // namespace

TEST_CASE("SoxBinary parses header correctly", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    REQUIRE(sox.load(data));
    REQUIRE(sox.version() == 100);
    REQUIRE(sox.recordCount() == 1);
}

TEST_CASE("SoxBinary parses troop fields", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    REQUIRE(sox.load(data));
    REQUIRE(sox.troops().size() == 1);

    const auto& troop = sox.troops()[0];
    REQUIRE_THAT(troop.moveSpeed, Catch::Matchers::WithinAbs(130.0f, 0.001f));
    REQUIRE_THAT(troop.resistMelee, Catch::Matchers::WithinAbs(100.0f, 0.001f));
    REQUIRE_THAT(troop.defaultUnitHp, Catch::Matchers::WithinAbs(800.0f, 0.001f));
}

TEST_CASE("SoxBinary round-trip preserves data", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto original = createMinimalTroopInfoSox();

    REQUIRE(sox.load(original));
    auto saved = sox.save();

    REQUIRE(saved.size() == original.size());
    REQUIRE(std::memcmp(saved.data(), original.data(), saved.size()) == 0);
}

TEST_CASE("SoxBinary validates resistance ranges", "[sox_binary]") {
    kuf::SoxBinary sox;
    auto data = createMinimalTroopInfoSox();

    // Set invalid resistance (outside -200 to 200, not in instant-death range).
    int32_t badResist = 500;
    std::memcpy(data.data() + 8 + 0x38, &badResist, 4);

    REQUIRE(sox.load(data));
    auto issues = sox.validate();

    REQUIRE(!issues.empty());
    REQUIRE(issues[0].severity == kuf::Severity::Warning);
}
