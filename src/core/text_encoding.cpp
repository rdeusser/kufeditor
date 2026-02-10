#include "core/text_encoding.h"

#include <iconv.h>

namespace kuf {

namespace {

std::string iconvConvert(const char* fromCode, const char* toCode,
                         const std::string& input) {
    if (input.empty()) return input;

    iconv_t cd = iconv_open(toCode, fromCode);
    if (cd == reinterpret_cast<iconv_t>(-1)) return input;

    size_t inLeft = input.size();
    size_t outLen = inLeft * 4;
    std::string output(outLen, '\0');

    char* inBuf = const_cast<char*>(input.data());
    char* outBuf = output.data();
    size_t outLeft = outLen;

    size_t result = iconv(cd, &inBuf, &inLeft, &outBuf, &outLeft);
    iconv_close(cd);

    if (result == static_cast<size_t>(-1)) return input;

    output.resize(outLen - outLeft);
    return output;
}

} // namespace

std::string cp949ToUtf8(const std::string& input) {
    return iconvConvert("CP949", "UTF-8", input);
}

std::string utf8ToCp949(const std::string& input) {
    return iconvConvert("UTF-8", "CP949", input);
}

} // namespace kuf
