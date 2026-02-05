#pragma once

#include "formats/file_format.h"

#include <string>
#include <vector>

namespace kuf {

struct TextEntry {
    uint8_t maxLength;
    std::string text;
};

class SoxText : public IFileFormat {
public:
    bool load(std::span<const std::byte> data) override;
    std::vector<std::byte> save() const override;
    std::string_view formatName() const override { return "Text SOX"; }
    GameVersion detectedVersion() const override { return version_; }
    std::vector<ValidationIssue> validate() const override;

    size_t entryCount() const { return entries_.size(); }
    const std::vector<TextEntry>& entries() const { return entries_; }
    std::vector<TextEntry>& entries() { return entries_; }

private:
    std::vector<TextEntry> entries_;
    GameVersion version_ = GameVersion::Unknown;
};

} // namespace kuf
