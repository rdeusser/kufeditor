#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_floating_point.hpp>

#include "formats/stg_format.h"
#include "formats/stg_script_catalog.h"

#include <cstring>
#include <vector>

namespace {

// Helper: append a uint32 (LE) to a byte vector.
void appendU32(std::vector<std::byte>& v, uint32_t val) {
    size_t pos = v.size();
    v.resize(pos + 4);
    std::memcpy(v.data() + pos, &val, 4);
}

// Helper: append an int32 (LE) to a byte vector.
void appendI32(std::vector<std::byte>& v, int32_t val) {
    size_t pos = v.size();
    v.resize(pos + 4);
    std::memcpy(v.data() + pos, &val, 4);
}

// Helper: append a float (LE) to a byte vector.
void appendF32(std::vector<std::byte>& v, float val) {
    size_t pos = v.size();
    v.resize(pos + 4);
    std::memcpy(v.data() + pos, &val, 4);
}

// Helper: append N zero bytes.
void appendZeros(std::vector<std::byte>& v, size_t count) {
    v.resize(v.size() + count, std::byte{0});
}

// Helper: append a fixed-size string (null-padded).
void appendFixedString(std::vector<std::byte>& v, const char* str, size_t fixedLen) {
    size_t pos = v.size();
    v.resize(pos + fixedLen, std::byte{0});
    size_t copyLen = std::min(std::strlen(str), fixedLen - 1);
    std::memcpy(v.data() + pos, str, copyLen);
}

// Helper: append a typed param value (matches ReadSTGParamValue format).
void appendParamInt(std::vector<std::byte>& v, int32_t val) {
    appendU32(v, 0); // type = Int
    appendI32(v, val);
}

void appendParamFloat(std::vector<std::byte>& v, float val) {
    appendU32(v, 1); // type = Float
    appendF32(v, val);
}

void appendParamString(std::vector<std::byte>& v, const char* str) {
    appendU32(v, 2); // type = String
    uint32_t len = static_cast<uint32_t>(std::strlen(str));
    appendU32(v, len);
    size_t pos = v.size();
    v.resize(pos + len);
    std::memcpy(v.data() + pos, str, len);
}

void appendParamEnum(std::vector<std::byte>& v, int32_t val) {
    appendU32(v, 3); // type = Enum
    appendI32(v, val);
}

// Build a minimal STG with header + 1 unit + empty tail.
std::vector<std::byte> createMinimalStg() {
    std::vector<std::byte> data(kuf::kStgHeaderSize + kuf::kStgUnitSize, std::byte{0});

    // Header: format magic = 0x3E9 (1001).
    uint32_t magic = 0x3E9;
    std::memcpy(data.data() + 0x000, &magic, 4);

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

    // Job type = 3 (HumanSpearman) at +0x54.
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

    // Unit anim config at +0x18C.
    uint32_t unitAnimConfig = 1;
    std::memcpy(unit + 0x18C, &unitAnimConfig, 4);

    // Grid X = 4 at +0x190 (uint32).
    uint32_t gridX = 4;
    std::memcpy(unit + 0x190, &gridX, 4);

    // Grid Y = 3 at +0x194 (uint32).
    uint32_t gridY = 3;
    std::memcpy(unit + 0x194, &gridY, 4);

    // TroopInfo index = 3 at +0x1C0 (int32).
    int32_t troopIdx = 3;
    std::memcpy(unit + 0x1C0, &troopIdx, 4);

    // Stat overrides: all -1.0 (22 floats at +0x1C8).
    for (int i = 0; i < 22; ++i) {
        std::memcpy(unit + 0x1C8 + i * 4, &negOne, 4);
    }

    return data;
}

// Build an 84-byte area entry.
void appendAreaEntry(std::vector<std::byte>& v, const char* desc, uint32_t areaId,
                     float x1, float y1, float x2, float y2) {
    size_t start = v.size();
    appendZeros(v, kuf::kStgAreaIdEntrySize);

    // Description at +0x00 (32 bytes).
    size_t copyLen = std::min(std::strlen(desc), size_t{31});
    std::memcpy(v.data() + start, desc, copyLen);

    // Area ID at +0x40.
    std::memcpy(v.data() + start + 0x40, &areaId, 4);

    // Bounds at +0x44, +0x48, +0x4C, +0x50.
    std::memcpy(v.data() + start + 0x44, &x1, 4);
    std::memcpy(v.data() + start + 0x48, &y1, 4);
    std::memcpy(v.data() + start + 0x4C, &x2, 4);
    std::memcpy(v.data() + start + 0x50, &y2, 4);
}

// Build a valid variable-length tail with: 0 area IDs, 0 variables, N events in 1 block, 0 footer entries.
std::vector<std::byte> createTail(const std::vector<std::vector<std::byte>>& eventBlobs) {
    std::vector<std::byte> tail;

    // Area IDs: count = 0.
    appendU32(tail, 0);

    // Variables: count = 0.
    appendU32(tail, 0);

    // Event block count = 1.
    appendU32(tail, 1);

    // Block header = 0.
    appendU32(tail, 0);

    // Event count.
    appendU32(tail, static_cast<uint32_t>(eventBlobs.size()));

    // Event data.
    for (const auto& blob : eventBlobs) {
        tail.insert(tail.end(), blob.begin(), blob.end());
    }

    // Footer: count = 0.
    appendU32(tail, 0);

    return tail;
}

// Build a variable-length event blob.
struct TestCondition {
    uint32_t typeId;
    std::vector<int32_t> intParams;
};

struct TestAction {
    uint32_t typeId;
    std::vector<int32_t> intParams;
};

std::vector<std::byte> buildEventBlob(const char* desc, uint32_t eventId,
    const std::vector<TestCondition>& conditions,
    const std::vector<TestAction>& actions) {

    std::vector<std::byte> buf;

    // Description (64 bytes).
    appendFixedString(buf, desc, 64);

    // Event ID.
    appendU32(buf, eventId);

    // Condition count.
    appendU32(buf, static_cast<uint32_t>(conditions.size()));

    // Conditions.
    for (const auto& c : conditions) {
        appendU32(buf, c.typeId);
        appendU32(buf, static_cast<uint32_t>(c.intParams.size()));
        for (int32_t p : c.intParams) {
            appendParamInt(buf, p);
        }
    }

    // Action count.
    appendU32(buf, static_cast<uint32_t>(actions.size()));

    // Actions.
    for (const auto& a : actions) {
        appendU32(buf, a.typeId);
        appendU32(buf, static_cast<uint32_t>(a.intParams.size()));
        for (int32_t p : a.intParams) {
            appendParamInt(buf, p);
        }
    }

    return buf;
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
    REQUIRE(stg.header().formatMagic == 0x3E9);
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

    REQUIRE(unit.leaderJobType == 3);
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
    REQUIRE(unit.unitAnimConfig == 1);
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
    stg.units()[0].leaderJobType = 6;
    stg.units()[0].leaderLevel = 10;
    stg.units()[0].positionX = 9999.0f;

    auto saved = stg.save();

    // Reload and verify.
    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));

    const auto& unit = stg2.units()[0];
    REQUIRE(unit.unitName == "ModifiedUnit");
    REQUIRE(unit.leaderJobType == 6);
    REQUIRE(unit.leaderLevel == 10);
    REQUIRE_THAT(unit.positionX, Catch::Matchers::WithinAbs(9999.0f, 0.001f));
}

TEST_CASE("StgFormat validates duplicate unique IDs", "[stg]") {
    // Create data with 2 units sharing the same unique ID.
    std::vector<std::byte> data(kuf::kStgHeaderSize + 2 * kuf::kStgUnitSize, std::byte{0});

    uint32_t magic = 0x3E9;
    std::memcpy(data.data() + 0x000, &magic, 4);

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

TEST_CASE("StgFormat preserves raw tail on round-trip", "[stg]") {
    auto data = createMinimalStg();

    // Append some fake tail data that won't parse.
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

TEST_CASE("StgFormat parses events from tail", "[stg][events]") {
    auto stgData = createMinimalStg();

    TestCondition cond{19, {1, 100, 2}};
    TestAction act1{90, {5}};
    TestAction act2{145, {5}};

    auto blob = buildEventBlob("Make Regnier Immortal", 1, {cond}, {act1, act2});
    auto tail = createTail({blob});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());
    REQUIRE(stg.eventBlocks().size() == 1);
    REQUIRE(stg.eventBlocks()[0].events.size() == 1);

    const auto& event = stg.eventBlocks()[0].events[0];
    REQUIRE(event.description == "Make Regnier Immortal");
    REQUIRE(event.eventId == 1);
    REQUIRE(event.conditions.size() == 1);
    REQUIRE(event.actions.size() == 2);

    REQUIRE(event.conditions[0].typeId == 19);
    REQUIRE(event.conditions[0].params.size() == 3);
    REQUIRE(event.conditions[0].params[0].intValue == 1);
    REQUIRE(event.conditions[0].params[1].intValue == 100);
    REQUIRE(event.conditions[0].params[2].intValue == 2);

    REQUIRE(event.actions[0].typeId == 90);
    REQUIRE(event.actions[0].params.size() == 1);
    REQUIRE(event.actions[0].params[0].intValue == 5);
    REQUIRE(event.actions[1].typeId == 145);
    REQUIRE(event.actions[1].params[0].intValue == 5);
}

TEST_CASE("StgFormat event round-trip preserves bytes", "[stg][events]") {
    auto stgData = createMinimalStg();

    TestCondition cond{6, {30}};
    TestAction act{3, {}};
    auto blob = buildEventBlob("Timer Win", 2, {cond}, {act});
    auto tail = createTail({blob});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat event round-trip with modifications", "[stg][events]") {
    auto stgData = createMinimalStg();

    TestCondition cond{0, {}};
    TestAction act{6, {1}};
    auto blob = buildEventBlob("Show Message", 0, {cond}, {act});
    auto tail = createTail({blob});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));

    // Modify the action parameter.
    stg.eventBlocks()[0].events[0].actions[0].params[0].intValue = 42;
    stg.eventBlocks()[0].events[0].modified = true;

    auto saved = stg.save();

    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));
    REQUIRE(stg2.tailParsed());
    REQUIRE(stg2.eventBlocks()[0].events[0].actions[0].params[0].intValue == 42);
    REQUIRE(stg2.eventBlocks()[0].events[0].description == "Show Message");
}

TEST_CASE("StgFormat event add and remove", "[stg][events]") {
    auto stgData = createMinimalStg();

    TestAction act{3, {}};
    auto blob = buildEventBlob("Win", 0, {}, {act});
    auto tail = createTail({blob});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.eventBlocks()[0].events.size() == 1);

    // Add a condition to the event.
    kuf::StgScriptEntry newCond;
    newCond.typeId = 19;
    kuf::StgParamValue pv;
    pv.type = kuf::StgParamType::Int;
    pv.intValue = 0;
    newCond.params.push_back(pv);
    pv.intValue = 1;
    newCond.params.push_back(pv);
    pv.intValue = 0;
    newCond.params.push_back(pv);
    stg.eventBlocks()[0].events[0].conditions.push_back(newCond);
    stg.eventBlocks()[0].events[0].modified = true;

    auto saved = stg.save();
    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));
    REQUIRE(stg2.eventBlocks()[0].events[0].conditions.size() == 1);
    REQUIRE(stg2.eventBlocks()[0].events[0].conditions[0].typeId == 19);
    REQUIRE(stg2.eventBlocks()[0].events[0].actions.size() == 1);

    // Remove the action.
    stg2.eventBlocks()[0].events[0].actions.clear();
    stg2.eventBlocks()[0].events[0].modified = true;

    auto saved2 = stg2.save();
    kuf::StgFormat stg3;
    REQUIRE(stg3.load(saved2));
    REQUIRE(stg3.eventBlocks()[0].events[0].actions.empty());
    REQUIRE(stg3.eventBlocks()[0].events[0].conditions.size() == 1);
}

TEST_CASE("StgFormat multiple events round-trip", "[stg][events]") {
    auto stgData = createMinimalStg();

    auto blob1 = buildEventBlob("Event A", 0, {}, {{3, {}}});
    auto blob2 = buildEventBlob("Event B", 1, {{0, {}}}, {{4, {}}});
    auto blob3 = buildEventBlob("Event C", 2, {{6, {60}}}, {});
    auto tail = createTail({blob1, blob2, blob3});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.totalEventCount() == 3);

    REQUIRE(stg.eventBlocks()[0].events[0].description == "Event A");
    REQUIRE(stg.eventBlocks()[0].events[1].description == "Event B");
    REQUIRE(stg.eventBlocks()[0].events[2].description == "Event C");

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat unparseable tail falls back to raw", "[stg][events]") {
    auto stgData = createMinimalStg();

    // Append garbage that won't parse as a valid tail.
    std::vector<std::byte> junk(100, std::byte{0xAB});
    stgData.insert(stgData.end(), junk.begin(), junk.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE_FALSE(stg.tailParsed());

    // Round-trip should still preserve everything.
    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat empty events section", "[stg][events]") {
    auto stgData = createMinimalStg();

    auto tail = createTail({});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());
    REQUIRE(stg.totalEventCount() == 0);

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat string params round-trip", "[stg][events]") {
    auto stgData = createMinimalStg();

    // Build an event with a string parameter manually.
    std::vector<std::byte> blob;
    appendFixedString(blob, "String Param Test", 64);
    appendU32(blob, 0); // event ID

    // 0 conditions.
    appendU32(blob, 0);

    // 1 action with 1 string param.
    appendU32(blob, 1);
    appendU32(blob, 6);  // ACT_SHOW_MESSAGE
    appendU32(blob, 1);  // 1 param
    appendParamString(blob, "hello world");

    auto tail = createTail({blob});
    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());

    const auto& act = stg.eventBlocks()[0].events[0].actions[0];
    REQUIRE(act.typeId == 6);
    REQUIRE(act.params.size() == 1);
    REQUIRE(act.params[0].type == kuf::StgParamType::String);
    REQUIRE(act.params[0].stringValue == "hello world");

    // Round-trip.
    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat footer with entries round-trip", "[stg][events]") {
    auto stgData = createMinimalStg();

    std::vector<std::byte> tail;

    // Area IDs: count = 0.
    appendU32(tail, 0);

    // Variables: count = 0.
    appendU32(tail, 0);

    // Event block count = 1, block header = 0, event count = 0.
    appendU32(tail, 1);
    appendU32(tail, 0);
    appendU32(tail, 0);

    // Footer: 3 entries.
    appendU32(tail, 3);
    appendU32(tail, 100); appendU32(tail, 200);
    appendU32(tail, 300); appendU32(tail, 400);
    appendU32(tail, 500); appendU32(tail, 600);

    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());
    REQUIRE(stg.footerEntries().size() == 3);
    REQUIRE(stg.footerEntries()[0].field1 == 100);
    REQUIRE(stg.footerEntries()[0].field2 == 200);
    REQUIRE(stg.footerEntries()[2].field1 == 500);
    REQUIRE(stg.footerEntries()[2].field2 == 600);

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat negative troopInfoIndex", "[stg]") {
    auto data = createMinimalStg();

    // Set troopInfoIndex to -1.
    int32_t negIdx = -1;
    std::byte* unit = data.data() + kuf::kStgHeaderSize;
    std::memcpy(unit + 0x1C0, &negIdx, 4);

    kuf::StgFormat stg;
    REQUIRE(stg.load(data));
    REQUIRE(stg.units()[0].troopInfoIndex == -1);

    // Round-trip.
    auto saved = stg.save();
    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));
    REQUIRE(stg2.units()[0].troopInfoIndex == -1);
}

TEST_CASE("StgFormat variable section round-trip", "[stg][events]") {
    auto stgData = createMinimalStg();

    std::vector<std::byte> tail;

    // Area IDs: count = 0.
    appendU32(tail, 0);

    // Variables: count = 2.
    appendU32(tail, 2);

    // Variable 0: "stage", id=0, initial value = Int(0).
    appendFixedString(tail, "stage", 64);
    appendU32(tail, 0);
    appendParamInt(tail, 0);

    // Variable 1: "cnt", id=1, initial value = Int(5).
    appendFixedString(tail, "cnt", 64);
    appendU32(tail, 1);
    appendParamInt(tail, 5);

    // Event block count = 1, block header = 0, event count = 0.
    appendU32(tail, 1);
    appendU32(tail, 0);
    appendU32(tail, 0);

    // Footer: count = 0.
    appendU32(tail, 0);

    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());
    REQUIRE(stg.variables().size() == 2);

    REQUIRE(stg.variables()[0].name == "stage");
    REQUIRE(stg.variables()[0].variableId == 0);
    REQUIRE(stg.variables()[0].initialValue.type == kuf::StgParamType::Int);
    REQUIRE(stg.variables()[0].initialValue.intValue == 0);

    REQUIRE(stg.variables()[1].name == "cnt");
    REQUIRE(stg.variables()[1].variableId == 1);
    REQUIRE(stg.variables()[1].initialValue.intValue == 5);

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat parses area entries", "[stg][areas]") {
    auto stgData = createMinimalStg();

    std::vector<std::byte> tail;

    // Area section: 2 entries.
    appendU32(tail, 2);
    appendAreaEntry(tail, "spawn_zone", 0, 1000.0f, 2000.0f, 3000.0f, 4000.0f);
    appendAreaEntry(tail, "objective", 1, 500.0f, 600.0f, 700.0f, 800.0f);

    // Variables: count = 0.
    appendU32(tail, 0);

    // Event block count = 1, block header = 0, event count = 0.
    appendU32(tail, 1);
    appendU32(tail, 0);
    appendU32(tail, 0);

    // Footer: count = 0.
    appendU32(tail, 0);

    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());
    REQUIRE(stg.areas().size() == 2);

    REQUIRE(stg.areas()[0].description == "spawn_zone");
    REQUIRE(stg.areas()[0].areaId == 0);
    REQUIRE_THAT(stg.areas()[0].boundX1, Catch::Matchers::WithinAbs(1000.0f, 0.001f));
    REQUIRE_THAT(stg.areas()[0].boundY1, Catch::Matchers::WithinAbs(2000.0f, 0.001f));
    REQUIRE_THAT(stg.areas()[0].boundX2, Catch::Matchers::WithinAbs(3000.0f, 0.001f));
    REQUIRE_THAT(stg.areas()[0].boundY2, Catch::Matchers::WithinAbs(4000.0f, 0.001f));

    REQUIRE(stg.areas()[1].description == "objective");
    REQUIRE(stg.areas()[1].areaId == 1);
    REQUIRE_THAT(stg.areas()[1].boundX1, Catch::Matchers::WithinAbs(500.0f, 0.001f));
    REQUIRE_THAT(stg.areas()[1].boundY2, Catch::Matchers::WithinAbs(800.0f, 0.001f));
}

TEST_CASE("StgFormat area round-trip preserves bytes", "[stg][areas]") {
    auto stgData = createMinimalStg();

    std::vector<std::byte> tail;

    // Area section: 1 entry.
    appendU32(tail, 1);
    appendAreaEntry(tail, "test_area", 42, 100.0f, 200.0f, 300.0f, 400.0f);

    // Variables: count = 0.
    appendU32(tail, 0);

    // Event block count = 1, block header = 0, event count = 0.
    appendU32(tail, 1);
    appendU32(tail, 0);
    appendU32(tail, 0);

    // Footer: count = 0.
    appendU32(tail, 0);

    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));
    REQUIRE(stg.tailParsed());

    auto saved = stg.save();
    REQUIRE(saved.size() == stgData.size());
    REQUIRE(std::memcmp(saved.data(), stgData.data(), saved.size()) == 0);
}

TEST_CASE("StgFormat area modification round-trip", "[stg][areas]") {
    auto stgData = createMinimalStg();

    std::vector<std::byte> tail;

    // Area section: 1 entry.
    appendU32(tail, 1);
    appendAreaEntry(tail, "old_name", 0, 0.0f, 0.0f, 0.0f, 0.0f);

    // Variables: count = 0.
    appendU32(tail, 0);

    // Event block count = 1, block header = 0, event count = 0.
    appendU32(tail, 1);
    appendU32(tail, 0);
    appendU32(tail, 0);

    // Footer: count = 0.
    appendU32(tail, 0);

    stgData.insert(stgData.end(), tail.begin(), tail.end());

    kuf::StgFormat stg;
    REQUIRE(stg.load(stgData));

    // Modify the area.
    stg.areas()[0].description = "new_name";
    stg.areas()[0].areaId = 99;
    stg.areas()[0].boundX1 = 1111.0f;

    auto saved = stg.save();

    kuf::StgFormat stg2;
    REQUIRE(stg2.load(saved));
    REQUIRE(stg2.tailParsed());
    REQUIRE(stg2.areas()[0].description == "new_name");
    REQUIRE(stg2.areas()[0].areaId == 99);
    REQUIRE_THAT(stg2.areas()[0].boundX1, Catch::Matchers::WithinAbs(1111.0f, 0.001f));
}

TEST_CASE("Script catalog lookups", "[stg][catalog]") {
    auto* condInfo = kuf::findConditionInfo(19);
    REQUIRE(condInfo != nullptr);
    REQUIRE(std::string(condInfo->name) == "CON_VAR_INT_COMPARE");
    REQUIRE(condInfo->paramCount == 3);
    REQUIRE(std::string(condInfo->paramNames[0]) == "VariableID");

    auto* actInfo = kuf::findActionInfo(90);
    REQUIRE(actInfo != nullptr);
    REQUIRE(std::string(actInfo->name) == "ACT_LEADER_INVULNERABLE");
    REQUIRE(actInfo->paramCount == 1);
    REQUIRE(std::string(actInfo->paramNames[0]) == "TroopID");

    REQUIRE(kuf::findConditionInfo(9999) == nullptr);
    REQUIRE(kuf::findActionInfo(9999) == nullptr);
}
