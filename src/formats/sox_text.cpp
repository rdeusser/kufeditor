#include "formats/sox_text.h"

#include <algorithm>
#include <cstring>

namespace kuf {

namespace {

template<typename T>
T readLE(const std::byte* data) {
    T value;
    std::memcpy(&value, data, sizeof(T));
    return value;
}

constexpr size_t HEADER_SIZE = 8;

} // namespace

bool SoxText::load(std::span<const std::byte> data) {
    if (data.size() < HEADER_SIZE) {
        return false;
    }

    // Check for header (version=100 like binary SOX).
    int32_t version = readLE<int32_t>(data.data());
    if (version != 100) {
        return false;
    }

    int32_t count = readLE<int32_t>(data.data() + 4);
    if (count < 0 || count > 10000) {
        return false;
    }

    entries_.clear();
    size_t offset = HEADER_SIZE;

    // Each entry: 4-byte index + 2-byte length (LE) + text bytes.
    while (offset + 6 <= data.size() && entries_.size() < static_cast<size_t>(count)) {
        // Skip 4-byte index.
        offset += 4;

        // Read 2-byte length.
        uint16_t textLen = readLE<uint16_t>(data.data() + offset);
        offset += 2;

        // Reject entries with zero length.
        if (textLen == 0) {
            entries_.clear();
            return false;
        }

        if (offset + textLen > data.size()) {
            break;
        }

        TextEntry entry;
        entry.maxLength = textLen;

        const char* textStart = reinterpret_cast<const char*>(data.data() + offset);
        entry.text = std::string(textStart, textLen);

        // Validate text is printable ASCII (plus common whitespace).
        for (char c : entry.text) {
            if (c != '\t' && c != '\n' && c != '\r' && (c < 32 || c > 126)) {
                entries_.clear();
                return false;
            }
        }

        entries_.push_back(std::move(entry));
        offset += textLen;
    }

    version_ = GameVersion::Crusaders;
    return !entries_.empty();
}

std::vector<std::byte> SoxText::save() const {
    std::vector<std::byte> data;

    // Write header.
    int32_t version = 100;
    int32_t count = static_cast<int32_t>(entries_.size());
    data.resize(HEADER_SIZE);
    std::memcpy(data.data(), &version, sizeof(version));
    std::memcpy(data.data() + 4, &count, sizeof(count));

    for (size_t i = 0; i < entries_.size(); ++i) {
        const auto& entry = entries_[i];

        // Write 4-byte index.
        int32_t index = static_cast<int32_t>(i);
        size_t pos = data.size();
        data.resize(pos + 4);
        std::memcpy(data.data() + pos, &index, sizeof(index));

        // Write 2-byte length.
        uint16_t textLen = static_cast<uint16_t>(entry.text.size());
        pos = data.size();
        data.resize(pos + 2);
        std::memcpy(data.data() + pos, &textLen, sizeof(textLen));

        // Write text bytes.
        for (char c : entry.text) {
            data.push_back(static_cast<std::byte>(c));
        }
    }

    return data;
}

std::vector<ValidationIssue> SoxText::validate() const {
    std::vector<ValidationIssue> issues;

    for (size_t i = 0; i < entries_.size(); ++i) {
        const auto& entry = entries_[i];

        if (entry.text.size() > entry.maxLength) {
            issues.push_back({
                Severity::Error,
                "text",
                "Text exceeds maximum length",
                i
            });
        }

        // Check for non-printable characters.
        for (char c : entry.text) {
            if (c != '\0' && (c < 32 || c > 126)) {
                issues.push_back({
                    Severity::Warning,
                    "text",
                    "Contains non-printable characters",
                    i
                });
                break;
            }
        }
    }

    return issues;
}

} // namespace kuf
