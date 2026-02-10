#include <catch2/catch_test_macros.hpp>

#include "formats/sox_skill_info.h"

#include <cstring>

namespace {

void writeLE16(std::vector<std::byte>& buf, uint16_t val) {
    buf.push_back(static_cast<std::byte>(val & 0xFF));
    buf.push_back(static_cast<std::byte>((val >> 8) & 0xFF));
}

void writeLE32(std::vector<std::byte>& buf, int32_t val) {
    uint32_t u;
    std::memcpy(&u, &val, 4);
    buf.push_back(static_cast<std::byte>(u & 0xFF));
    buf.push_back(static_cast<std::byte>((u >> 8) & 0xFF));
    buf.push_back(static_cast<std::byte>((u >> 16) & 0xFF));
    buf.push_back(static_cast<std::byte>((u >> 24) & 0xFF));
}

void writeLE32U(std::vector<std::byte>& buf, uint32_t val) {
    buf.push_back(static_cast<std::byte>(val & 0xFF));
    buf.push_back(static_cast<std::byte>((val >> 8) & 0xFF));
    buf.push_back(static_cast<std::byte>((val >> 16) & 0xFF));
    buf.push_back(static_cast<std::byte>((val >> 24) & 0xFF));
}

void writeString(std::vector<std::byte>& buf, const std::string& str) {
    writeLE16(buf, static_cast<uint16_t>(str.size()));
    for (char c : str) {
        buf.push_back(static_cast<std::byte>(c));
    }
}

void writeFooter(std::vector<std::byte>& buf) {
    std::string footer = "THEND";
    for (char c : footer) {
        buf.push_back(static_cast<std::byte>(c));
    }
    for (size_t i = footer.size(); i < 64; ++i) {
        buf.push_back(static_cast<std::byte>(' '));
    }
}

std::vector<std::byte> createSkillInfoSox(
    int32_t id, const std::string& locKey, const std::string& iconPath,
    uint32_t slotCount, uint32_t maxLevel) {

    std::vector<std::byte> data;

    // Header.
    writeLE32(data, 100);   // version
    writeLE32(data, 1);     // count

    // Record.
    writeLE32(data, id);
    writeString(data, locKey);
    writeString(data, iconPath);
    writeLE32U(data, slotCount);
    writeLE32U(data, maxLevel);

    // Footer.
    writeFooter(data);

    return data;
}

std::vector<std::byte> createTwoSkillSox() {
    std::vector<std::byte> data;

    writeLE32(data, 100);
    writeLE32(data, 2);

    // Skill 0: Melee.
    writeLE32(data, 0);
    writeString(data, "@(S_Melee)");
    writeString(data, "IL_SKL_Melee.tga");
    writeLE32U(data, 1);
    writeLE32U(data, 50);

    // Skill 1: Fire.
    writeLE32(data, 8);
    writeString(data, "@(S_Fire)");
    writeString(data, "IL_SKL_Fire.tga");
    writeLE32U(data, 2);
    writeLE32U(data, 25);

    writeFooter(data);

    return data;
}

} // namespace

TEST_CASE("SoxSkillInfo parses header correctly", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "@(S_Melee)", "IL_SKL_Melee.tga", 1, 50);

    REQUIRE(sox.load(data));
    REQUIRE(sox.version() == 100);
    REQUIRE(sox.recordCount() == 1);
}

TEST_CASE("SoxSkillInfo parses skill fields", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "@(S_Melee)", "IL_SKL_Melee.tga", 1, 50);

    REQUIRE(sox.load(data));
    REQUIRE(sox.skills().size() == 1);

    const auto& skill = sox.skills()[0];
    REQUIRE(skill.id == 0);
    REQUIRE(skill.locKey == "@(S_Melee)");
    REQUIRE(skill.iconPath == "IL_SKL_Melee.tga");
    REQUIRE(skill.slotCount == 1);
    REQUIRE(skill.maxLevel == 50);
}

TEST_CASE("SoxSkillInfo round-trip preserves data", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto original = createTwoSkillSox();

    REQUIRE(sox.load(original));
    auto saved = sox.save();

    REQUIRE(saved.size() == original.size());
    REQUIRE(std::memcmp(saved.data(), original.data(), saved.size()) == 0);
}

TEST_CASE("SoxSkillInfo handles negative skill ID", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(-2, "@(S_Elemental)", "IL_SKL_Elem.tga", 2, 25);

    REQUIRE(sox.load(data));
    REQUIRE(sox.skills()[0].id == -2);

    auto saved = sox.save();
    REQUIRE(saved.size() == data.size());
    REQUIRE(std::memcmp(saved.data(), data.data(), saved.size()) == 0);
}

TEST_CASE("SoxSkillInfo validates out-of-range slot count", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "@(S_Melee)", "IL_SKL_Melee.tga", 5, 50);

    REQUIRE(sox.load(data));
    auto issues = sox.validate();

    REQUIRE(!issues.empty());
    bool foundSlotWarning = false;
    for (const auto& issue : issues) {
        if (issue.field == "slotCount" && issue.severity == kuf::Severity::Warning) {
            foundSlotWarning = true;
        }
    }
    REQUIRE(foundSlotWarning);
}

TEST_CASE("SoxSkillInfo validates zero max level", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "@(S_Melee)", "IL_SKL_Melee.tga", 1, 0);

    REQUIRE(sox.load(data));
    auto issues = sox.validate();

    REQUIRE(!issues.empty());
    bool foundLevelWarning = false;
    for (const auto& issue : issues) {
        if (issue.field == "maxLevel" && issue.severity == kuf::Severity::Warning) {
            foundLevelWarning = true;
        }
    }
    REQUIRE(foundLevelWarning);
}

TEST_CASE("SoxSkillInfo warns on empty strings", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "", "", 1, 50);

    REQUIRE(sox.load(data));
    auto issues = sox.validate();

    int warningCount = 0;
    for (const auto& issue : issues) {
        if (issue.field == "locKey" || issue.field == "iconPath") {
            warningCount++;
        }
    }
    REQUIRE(warningCount == 2);
}

TEST_CASE("SoxSkillInfo rejects truncated data", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;

    // Too small to even hold header + footer.
    std::vector<std::byte> tiny(10, std::byte{0});
    REQUIRE_FALSE(sox.load(tiny));
}

TEST_CASE("SoxSkillInfo rejects wrong version", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createSkillInfoSox(0, "@(S_Melee)", "IL_SKL_Melee.tga", 1, 50);

    // Overwrite version to 200.
    int32_t badVersion = 200;
    std::memcpy(data.data(), &badVersion, 4);

    REQUIRE_FALSE(sox.load(data));
}

TEST_CASE("SoxSkillInfo parses multiple records", "[sox_skill_info]") {
    kuf::SoxSkillInfo sox;
    auto data = createTwoSkillSox();

    REQUIRE(sox.load(data));
    REQUIRE(sox.recordCount() == 2);

    REQUIRE(sox.skills()[0].id == 0);
    REQUIRE(sox.skills()[0].locKey == "@(S_Melee)");
    REQUIRE(sox.skills()[0].slotCount == 1);
    REQUIRE(sox.skills()[0].maxLevel == 50);

    REQUIRE(sox.skills()[1].id == 8);
    REQUIRE(sox.skills()[1].locKey == "@(S_Fire)");
    REQUIRE(sox.skills()[1].slotCount == 2);
    REQUIRE(sox.skills()[1].maxLevel == 25);
}
