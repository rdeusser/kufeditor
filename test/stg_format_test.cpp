#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_floating_point.hpp>

#include "formats/stg_format.h"

#include <cstring>
#include <vector>

namespace {

// Build a minimal STG with header + 1 unit + empty tail.
std::vector<std::byte> createMinimalStg() {
    std::vector<std::byte> data(kuf::kStgHeaderSize + kuf::kStgUnitSize, std::byte{0});

    // Header: mission ID = 1001.
    uint32_t missionId = 1001;
    std::memcpy(data.data() + 0x000, &missionId, 4);

    // Map filename at 0x048.
    const char* mapName = "E1001.map";
    std::memcpy(data.data() + 0x048, mapName, std::strlen(mapName));

    // Unit count = 1 at 0x270.
    uint32_t unitCount = 1;
    std::memcpy(data.data() + 0x270, &unitCount, 4);

    // Unit data starts at offset kStgHeaderSize (628).
    std::byte* unit = data.data() + kuf::kStgHeaderSize;

    // Unit name at +0x00.
    const char* unitName = "TestUnit";
    std::memcpy(unit + 0x00, unitName, std::strlen(unitName));

    // Unique ID = 42 at +0x20.
    uint32_t uniqueId = 42;
    std::memcpy(unit + 0x20, &uniqueId, 4);

    // UCD = Enemy (1) at +0x24.
    unit[0x24] = std::byte{1};

    // IsEnabled = 1 at +0x26.
    unit[0x26] = std::byte{1};

    // Leader HP override = -1.0 at +0x28.
    float negOne = -1.0f;
    std::memcpy(unit + 0x28, &negOne, 4);

    // Unit HP override = -1.0 at +0x2C.
    std::memcpy(unit + 0x2C, &negOne, 4);

    // Position X = 5000.0 at +0x44.
    float posX = 5000.0f;
    std::memcpy(unit + 0x44, &posX, 4);

    // Position Y = 3000.0 at +0x48.
    float posY = 3000.0f;
    std::memcpy(unit + 0x48, &posY, 4);

    // Direction = North (2) at +0x4C.
    unit[0x4C] = std::byte{2};

    // Job type = HumanSpearman (3) at +0x54.
    unit[0x54] = std::byte{3};

    // Model ID = 0 at +0x55.
    unit[0x55] = std::byte{0};

    // Worldmap ID = 0xFF at +0x56.
    unit[0x56] = std::byte{0xFF};

    // Level = 5 at +0x57.
    unit[0x57] = std::byte{5};

    // Officer count = 0 at +0xBC.
    uint32_t officerCount = 0;
    std::memcpy(unit + 0xBC, &officerCount, 4);

    // TroopInfo index = 3 at +0x1C0.
    uint32_t troopIdx = 3;
    std::memcpy(unit + 0x1C0, &troopIdx, 4);

    // Grid X = 4 at +0x190.
    uint32_t gridX = 4;
    std::memcpy(unit + 0x190, &gridX, 4);

    // Grid Y = 3 at +0x194.
    uint32_t gridY = 3;
    std::memcpy(unit + 0x194, &gridY, 4);

    // Stat overrides: all -1.0 (22 floats at +0x1C8).
    for (int i = 0; i < 22; ++i) {
        std::memcpy(unit + 0x1C8 + i * 4, &negOne, 4);
    }

    return data;
}

} // namespace

TEST_CASE("StgFormat rejects data smaller than header", "[stg]") {
    kuf::StgFormat stg;
    std::vector<std::byte> tooSmall(100, std::byte{0});
    REQUIRE_FALSE(stg.load(tooSmall));
}

TEST_CASE("StgFormat parses header correctly", "[stg]") {
    kuf::StgFormat stg;
    auto data = createMinimalStg();

    REQUIRE(stg.load(data));
    REQUIRE(stg.header().missionId == 1001);
    REQUIRE(stg.header().mapFile == "E1001.map");
    REQUIRE(stg.header().unitCount == 1);
    REQUIRE(stg.detectedVersion() == kuf::GameVersion::Crusaders);
}

TEST_CASE("StgFormat parses unit core data", "[stg]") {
    kuf::StgFormat stg;
    auto data = createMinimalStg();

    REQUIRE(stg.load(data));
    REQUIRE(stg.unitCount() == 1);

    const auto& unit = stg.units()[0];
    REQUIRE(unit.unitName == "TestUnit");
    REQUIRE(unit.uniqueId == 42);
    REQUIRE(unit.ucd == kuf::UCD::Enemy);
    REQUIRE(unit.isEnabled == 1);
    REQUIRE(unit.isHero == 0);
    REQUIRE_THAT(unit.leaderHpOverride, Catch::Matchers::WithinAbs(-1.0f, 0.001f));
    REQUIRE_THAT(unit.unitHpOverride, Catch::Matchers::WithinAbs(-1.0f, 0.001f));
    REQUIRE_THAT(unit.positionX, Catch::Matchers::WithinAbs(5000.0f, 0.001f));
    REQUIRE_THAT(unit.positionY, Catch::Matchers::WithinAbs(3000.0f, 0.001f));
    REQUIRE(unit.direction == kuf::Direction::North);
}

TEST_CASE("StgFormat parses leader configuration", "[stg]") {
    kuf::StgFormat stg;
    auto data = createMinimalStg();

    REQUIRE(stg.load(data));
    const auto& unit = stg.units()[0];

    REQUIRE(unit.leaderJobType == kuf::JobType::HumanSpearman);
    REQUIRE(unit.leaderModelId == 0);
    REQUIRE(unit.leaderWorldmapId == 0xFF);
    REQUIRE(unit.leaderLevel == 5);
    REQUIRE(unit.officerCount == 0);
}

TEST_CASE("StgFormat parses unit configuration", "[stg]") {
    kuf::StgFormat stg;
    auto data = createMinimalStg();

    REQUIRE(stg.load(data));
    const auto& unit = stg.units()[0];

    REQUIRE(unit.troopInfoIndex == 3);
    REQUIRE(unit.gridX == 4);
    REQUIRE(unit.gridY == 3);

    // All stat overrides should be -1.0.
    for (int i = 0; i < 22; ++i) {
        REQUIRE_THAT(unit.statOverrides[i], Catch::Matchers::WithinAbs(-1.0f, 0.001f));
    }
}

TEST_CASE("StgFormat round-trip preserves data", "[stg]") {
    kuf::StgFormat stg;
    auto original = createMinimalStg();

    REQUIRE(stg.load(original));
    auto saved = stg.save();

    REQUIRE(saved.size() == original.size());
    REQUIRE(std::memcmp(saved.data(), original.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat round-trip preserves modified fields", "[stg]") {
    kuf::StgFormat stg;
    auto data = createMinimalStg();

    REQUIRE(stg.load(data));

    // Modify some fields.
    stg.units()[0].unitName = "ModifiedUnit";
    stg.units()[0].leaderJobType = kuf::JobType::HumanPaladin;
    stg.units()[0].leaderLevel = 10;
    stg.units()[0].positionX = 9999.0f;

    auto saved = stg.save();

    // Reload and verify.
    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));

    const auto& unit = stg2.units()[0];
    REQUIRE(unit.unitName == "ModifiedUnit");
    REQUIRE(unit.leaderJobType == kuf::JobType::HumanPaladin);
    REQUIRE(unit.leaderLevel == 10);
    REQUIRE_THAT(unit.positionX, Catch::Matchers::WithinAbs(9999.0f, 0.001f));
}

TEST_CASE("StgFormat validates duplicate unique IDs", "[stg]") {
    // Create data with 2 units sharing the same unique ID.
    std::vector<std::byte> data(kuf::kStgHeaderSize + 2 * kuf::kStgUnitSize, std::byte{0});

    uint32_t unitCount = 2;
    std::memcpy(data.data() + 0x270, &unitCount, 4);

    // Both units get unique ID = 1.
    uint32_t id = 1;
    std::byte* unit0 = data.data() + kuf::kStgHeaderSize;
    std::byte* unit1 = data.data() + kuf::kStgHeaderSize + kuf::kStgUnitSize;
    std::memcpy(unit0 + 0x20, &id, 4);
    std::memcpy(unit1 + 0x20, &id, 4);

    // Set enabled and valid level so we only test duplicate ID.
    unit0[0x26] = std::byte{1};
    unit1[0x26] = std::byte{1};
    unit0[0x57] = std::byte{1};
    unit1[0x57] = std::byte{1};

    // Set unit names to avoid "empty name" warnings.
    const char* name0 = "Unit0";
    const char* name1 = "Unit1";
    std::memcpy(unit0 + 0x00, name0, std::strlen(name0));
    std::memcpy(unit1 + 0x00, name1, std::strlen(name1));

    // Set worldmap IDs to 0xFF.
    unit0[0x56] = std::byte{0xFF};
    unit1[0x56] = std::byte{0xFF};

    kuf::StgFormat stg;
    REQUIRE(stg.load(data));

    auto issues = stg.validate();
    bool foundDuplicate = false;
    for (const auto& issue : issues) {
        if (issue.field == "uniqueId" && issue.severity == kuf::Severity::Error) {
            foundDuplicate = true;
            break;
        }
    }
    REQUIRE(foundDuplicate);
}

TEST_CASE("StgFormat validates invalid job type", "[stg]") {
    auto data = createMinimalStg();

    // Set job type to invalid value 99.
    std::byte* unit = data.data() + kuf::kStgHeaderSize;
    unit[0x54] = std::byte{99};

    kuf::StgFormat stg;
    REQUIRE(stg.load(data));

    auto issues = stg.validate();
    bool foundJobError = false;
    for (const auto& issue : issues) {
        if (issue.field == "leaderJobType" && issue.severity == kuf::Severity::Error) {
            foundJobError = true;
            break;
        }
    }
    REQUIRE(foundJobError);
}

TEST_CASE("StgFormat preserves raw tail on round-trip", "[stg]") {
    auto data = createMinimalStg();

    // Append some fake tail data (AreaIDs, Variables, Events, Footer).
    std::vector<std::byte> tail(100, std::byte{0xAB});
    data.insert(data.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(data));

    auto saved = stg.save();
    REQUIRE(saved.size() == data.size());
    REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat rejects truncated unit data", "[stg]") {
    auto data = createMinimalStg();

    // Truncate: header says 1 unit but data is too short.
    data.resize(kuf::kStgHeaderSize + 100);

    kuf::StgFormat stg;
    REQUIRE_FALSE(stg.load(data));
}
