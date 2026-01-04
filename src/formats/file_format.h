#pragma once

#include "formats/validation.h"

#include <cstddef>
#include <span>
#include <string_view>
#include <vector>

namespace kuf {

enum class GameVersion {
    Crusaders,
    Heroes,
    Unknown
};

class IFileFormat {
public:
    virtual ~IFileFormat() = default;

    virtual bool load(std::span<const std::byte> data) = 0;
    virtual std::vector<std::byte> save() const = 0;
    virtual std::string_view formatName() const = 0;
    virtual GameVersion detectedVersion() const = 0;
    virtual std::vector<ValidationIssue> validate() const = 0;
};

} // namespace kuf
