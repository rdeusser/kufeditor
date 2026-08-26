#pragma once

#include <optional>
#include <string>
#include <string_view>

namespace kuf {

// Convert a CP949 (Korean) encoded string to UTF-8.
// Returns the original string unchanged if conversion fails.
std::string cp949ToUtf8(const std::string &input);

// Convert a UTF-8 string to CP949 (Korean) encoding.
// Returns the original string unchanged if conversion fails.
std::string utf8ToCp949(const std::string &input);

// Validate UTF8 without replacing or discarding malformed input.
bool isValidUTF8(std::string_view input);

// Convert UTF8 to CP949. Returns no value if the input is malformed or cannot
// be represented in CP949.
std::optional<std::string> UTF8ToCP949Checked(std::string_view input);

} // namespace kuf
