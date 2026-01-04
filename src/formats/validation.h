#pragma once

#include <cstddef>
#include <string>

namespace kuf {

enum class Severity {
    Info,
    Warning,
    Error
};

struct ValidationIssue {
    Severity severity;
    std::string field;
    std::string message;
    size_t recordIndex;
};

} // namespace kuf
