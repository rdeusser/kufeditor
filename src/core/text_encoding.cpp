#include "core/text_encoding.h"

#include <iconv.h>

#include <optional>
#include <string_view>

namespace kuf {

namespace {

std::optional<std::string> iconvConvertChecked(const char *fromCode,
					       const char *toCode,
					       std::string_view input) {
	if (input.empty()) return std::string{};

	iconv_t cd = iconv_open(toCode, fromCode);
	if (cd == reinterpret_cast<iconv_t>(-1)) return std::nullopt;

	size_t inLeft = input.size();
	size_t outLen = inLeft * 4 + 4;
	std::string output(outLen, '\0');

	char *inBuf = const_cast<char *>(input.data());
	char *outBuf = output.data();
	size_t outLeft = outLen;

	size_t result = iconv(cd, &inBuf, &inLeft, &outBuf, &outLeft);
	iconv_close(cd);

	if (result == static_cast<size_t>(-1) || inLeft != 0) {
		return std::nullopt;
	}

	output.resize(outLen - outLeft);
	return output;
}

std::string iconvConvert(const char *fromCode, const char *toCode,
			 const std::string &input) {
	auto converted = iconvConvertChecked(fromCode, toCode, input);
	return converted.value_or(input);
}

} // namespace

std::string cp949ToUtf8(const std::string &input) {
	return iconvConvert("CP949", "UTF-8", input);
}

std::string utf8ToCp949(const std::string &input) {
	return iconvConvert("UTF-8", "CP949", input);
}

bool isValidUTF8(std::string_view input) {
	return iconvConvertChecked("UTF-8", "UTF-8", input).has_value();
}

std::optional<std::string> UTF8ToCP949Checked(std::string_view input) {
	if (!isValidUTF8(input)) return std::nullopt;
	return iconvConvertChecked("UTF-8", "CP949", input);
}

} // namespace kuf
