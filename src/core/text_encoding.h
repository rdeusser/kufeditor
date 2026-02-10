#pragma once

#include <string>

namespace kuf {

// Convert a CP949 (Korean) encoded string to UTF-8.
// Returns the original string unchanged if conversion fails.
std::string cp949ToUtf8(const std::string& input);

// Convert a UTF-8 string to CP949 (Korean) encoding.
// Returns the original string unchanged if conversion fails.
std::string utf8ToCp949(const std::string& input);

} // namespace kuf
