#include <catch2/catch_test_macros.hpp>

#include "core/application.h"
#include "core/tab_manager.h"

#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <vector>

namespace {

std::vector<std::byte> createSaveFlowSTG() {
	std::vector<std::byte> bytes(kuf::kStgHeaderSize + kuf::kStgUnitSize,
				     std::byte{0});
	const uint32_t magic = 0x3E9;
	const uint32_t unitCount = 1;
	std::memcpy(bytes.data(), &magic, sizeof(magic));
	std::memcpy(bytes.data() + 0x270, &unitCount, sizeof(unitCount));
	std::memcpy(bytes.data() + kuf::kStgHeaderSize, "unit", 4);
	return bytes;
}

std::filesystem::path saveFlowPath(const void *identity) {
	return std::filesystem::temp_directory_path() /
	       ("kufeditor-save-flow-" +
		std::to_string(reinterpret_cast<uintptr_t>(identity)) + ".stg");
}

void writeBytes(const std::filesystem::path &path,
		std::span<const std::byte> bytes) {
	std::ofstream output(path, std::ios::binary | std::ios::trunc);
	REQUIRE(output);
	output.write(reinterpret_cast<const char *>(bytes.data()),
		     static_cast<std::streamsize>(bytes.size()));
	REQUIRE(output.good());
}

std::vector<std::byte> readBytes(const std::filesystem::path &path) {
	std::ifstream input(path, std::ios::binary | std::ios::ate);
	REQUIRE(input);
	const auto size = input.tellg();
	REQUIRE(size >= 0);
	std::vector<std::byte> bytes(static_cast<size_t>(size));
	input.seekg(0);
	input.read(reinterpret_cast<char *>(bytes.data()), size);
	REQUIRE(input.good());
	return bytes;
}

class CloseFailingFile {
      public:
	void close() {
		closeCalled = true;
		failed = true;
	}

	bool fail() const { return failed; }

	bool closeCalled = false;

      private:
	bool failed = false;
};

} // namespace

TEST_CASE("Build system works", "[setup]") { REQUIRE(1 + 1 == 2); }

TEST_CASE("TabManager reports STG serialization failure before writing",
	  "[save-flow][stg]") {
	auto source = createSaveFlowSTG();
	auto format = std::make_shared<kuf::StgFormat>();
	REQUIRE(format->load(source));
	format->units()[0].unitName = "\xF0\x9F\x98\x80";
	format->units()[0].positionX = 42.0f;

	kuf::OpenDocument document;
	document.stgData = format;
	document.dirty = true;
	const auto path = saveFlowPath(&document);
	document.path = path.string();
	writeBytes(path, source);

	bool writerCalled = false;
	kuf::TabManager manager(
	    [&writerCalled](const std::string &, std::span<const std::byte>) {
		    writerCalled = true;
		    return false;
	    });
	std::string popupMessage;
	bool showPopup = false;
	kuf::bindApplicationSaveErrorPopup(manager, popupMessage, showPopup);

	const auto result = manager.saveDocument(&document);
	REQUIRE(result.status == kuf::SaveStatus::SerializationFailed);
	REQUIRE(result.STGError.has_value());
	REQUIRE(result.STGError->field == "units[0].unitName");
	REQUIRE_FALSE(writerCalled);
	REQUIRE(document.dirty);
	REQUIRE(readBytes(path) == source);
	REQUIRE(showPopup);
	REQUIRE(popupMessage == result.message);

	std::filesystem::remove(path);
}

TEST_CASE("TabManager keeps a document dirty after deterministic write failure",
	  "[save-flow][stg]") {
	auto source = createSaveFlowSTG();
	auto format = std::make_shared<kuf::StgFormat>();
	REQUIRE(format->load(source));
	format->units()[0].positionX = 42.0f;

	kuf::OpenDocument document;
	document.stgData = format;
	document.dirty = true;
	document.path = "writer-failure.stg";

	size_t writerCalls = 0;
	kuf::TabManager manager(
	    [&writerCalls](const std::string &,
			   std::span<const std::byte> bytes) {
		    ++writerCalls;
		    REQUIRE_FALSE(bytes.empty());
		    return false;
	    });
	std::optional<kuf::SaveResult> callbackResult;
	size_t callbackCalls = 0;
	manager.setOnSaveError(
	    [&callbackResult, &callbackCalls](const kuf::SaveResult &result) {
		    ++callbackCalls;
		    callbackResult = result;
	    });

	const auto result = manager.saveDocument(&document);
	REQUIRE(result.status == kuf::SaveStatus::WriteFailed);
	REQUIRE(writerCalls == 1);
	REQUIRE(document.dirty);
	REQUIRE(callbackResult.has_value());
	REQUIRE(callbackCalls == 1);
	REQUIRE(callbackResult->status == kuf::SaveStatus::WriteFailed);
	REQUIRE_FALSE(callbackResult->STGError.has_value());
	REQUIRE(callbackResult->message == result.message);
}

TEST_CASE("TabManager observes a close failure before reporting success",
	  "[save-flow]") {
	CloseFailingFile file;
	REQUIRE_FALSE(kuf::tab_manager_detail::finishFileWrite(file));
	REQUIRE(file.closeCalled);
}

TEST_CASE("TabManager clears dirty only after a complete write",
	  "[save-flow][stg]") {
	auto format = std::make_shared<kuf::StgFormat>();
	auto source = createSaveFlowSTG();
	REQUIRE(format->load(source));
	format->units()[0].positionX = 42.0f;

	kuf::OpenDocument document;
	document.stgData = format;
	document.dirty = true;
	document.path = "successful-write.stg";

	size_t writerCalls = 0;
	kuf::TabManager manager(
	    [&writerCalls](const std::string &,
			   std::span<const std::byte> bytes) {
		    ++writerCalls;
		    return !bytes.empty();
	    });
	const auto result = manager.saveDocument(&document);
	REQUIRE(result.status == kuf::SaveStatus::Saved);
	REQUIRE(writerCalls == 1);
	REQUIRE_FALSE(document.dirty);
}
