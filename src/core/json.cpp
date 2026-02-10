#include "core/json.h"
#include "mods/mod_manager.h"

#include <sstream>

namespace kuf {

namespace {

// Minimal JSON string parser: advances pos past the opening quote, reads until
// the closing quote, handling \" and \\ escapes. Returns the unescaped string.
std::string readJsonString(const std::string& text, size_t& pos) {
    if (pos >= text.size() || text[pos] != '"') return {};
    ++pos; // skip opening quote
    std::string result;
    while (pos < text.size() && text[pos] != '"') {
        if (text[pos] == '\\' && pos + 1 < text.size()) {
            ++pos;
            switch (text[pos]) {
                case '"': result += '"'; break;
                case '\\': result += '\\'; break;
                case '/': result += '/'; break;
                case 'n': result += '\n'; break;
                case 't': result += '\t'; break;
                case 'r': result += '\r'; break;
                default: result += text[pos]; break;
            }
        } else {
            result += text[pos];
        }
        ++pos;
    }
    if (pos < text.size()) ++pos; // skip closing quote
    return result;
}

void skipWhitespace(const std::string& text, size_t& pos) {
    while (pos < text.size() && (text[pos] == ' ' || text[pos] == '\t' ||
                                  text[pos] == '\n' || text[pos] == '\r')) {
        ++pos;
    }
}

// Reads a JSON string array: ["a", "b", "c"]
std::vector<std::string> readJsonStringArray(const std::string& text, size_t& pos) {
    std::vector<std::string> result;
    if (pos >= text.size() || text[pos] != '[') return result;
    ++pos; // skip '['
    skipWhitespace(text, pos);
    while (pos < text.size() && text[pos] != ']') {
        if (text[pos] == '"') {
            result.push_back(readJsonString(text, pos));
        } else {
            ++pos;
        }
        skipWhitespace(text, pos);
        if (pos < text.size() && text[pos] == ',') ++pos;
        skipWhitespace(text, pos);
    }
    if (pos < text.size()) ++pos; // skip ']'
    return result;
}

std::string escapeJsonString(const std::string& s) {
    std::string result;
    result.reserve(s.size() + 4);
    for (char c : s) {
        switch (c) {
            case '"': result += "\\\""; break;
            case '\\': result += "\\\\"; break;
            case '\n': result += "\\n"; break;
            case '\r': result += "\\r"; break;
            case '\t': result += "\\t"; break;
            default: result += c; break;
        }
    }
    return result;
}

} // namespace

std::optional<ModMetadata> parseModJson(const std::string& text) {
    ModMetadata meta;
    size_t pos = 0;

    skipWhitespace(text, pos);
    if (pos >= text.size() || text[pos] != '{') return std::nullopt;
    ++pos;

    while (pos < text.size() && text[pos] != '}') {
        skipWhitespace(text, pos);
        if (text[pos] != '"') { ++pos; continue; }

        std::string key = readJsonString(text, pos);
        skipWhitespace(text, pos);
        if (pos >= text.size() || text[pos] != ':') return std::nullopt;
        ++pos;
        skipWhitespace(text, pos);

        if (key == "files") {
            meta.files = readJsonStringArray(text, pos);
        } else if (pos < text.size() && text[pos] == '"') {
            std::string value = readJsonString(text, pos);
            if (key == "name") meta.name = value;
            else if (key == "version") meta.version = value;
            else if (key == "author") meta.author = value;
            else if (key == "description") meta.description = value;
            else if (key == "game") meta.game = value;
            else if (key == "created") meta.created = value;
        } else {
            // Skip unknown value types.
            while (pos < text.size() && text[pos] != ',' && text[pos] != '}') ++pos;
        }

        skipWhitespace(text, pos);
        if (pos < text.size() && text[pos] == ',') ++pos;
    }

    // Validate required fields.
    if (meta.name.empty() || meta.version.empty() || meta.game.empty() || meta.files.empty()) {
        return std::nullopt;
    }

    return meta;
}

std::string serializeModJson(const ModMetadata& meta) {
    std::ostringstream out;
    out << "{\n";
    out << "  \"name\": \"" << escapeJsonString(meta.name) << "\",\n";
    out << "  \"version\": \"" << escapeJsonString(meta.version) << "\",\n";
    if (!meta.author.empty()) {
        out << "  \"author\": \"" << escapeJsonString(meta.author) << "\",\n";
    }
    if (!meta.description.empty()) {
        out << "  \"description\": \"" << escapeJsonString(meta.description) << "\",\n";
    }
    out << "  \"game\": \"" << escapeJsonString(meta.game) << "\",\n";
    if (!meta.created.empty()) {
        out << "  \"created\": \"" << escapeJsonString(meta.created) << "\",\n";
    }
    out << "  \"files\": [\n";
    for (size_t i = 0; i < meta.files.size(); ++i) {
        out << "    \"" << escapeJsonString(meta.files[i]) << "\"";
        if (i + 1 < meta.files.size()) out << ",";
        out << "\n";
    }
    out << "  ]\n";
    out << "}\n";
    return out.str();
}

std::vector<InstalledModInfo> parseInstalledModsJson(const std::string& text) {
    std::vector<InstalledModInfo> result;
    size_t pos = 0;

    skipWhitespace(text, pos);
    if (pos >= text.size() || text[pos] != '{') return result;
    ++pos;

    // Find "mods" key.
    while (pos < text.size() && text[pos] != '}') {
        skipWhitespace(text, pos);
        if (text[pos] != '"') { ++pos; continue; }

        std::string key = readJsonString(text, pos);
        skipWhitespace(text, pos);
        if (pos >= text.size() || text[pos] != ':') return result;
        ++pos;
        skipWhitespace(text, pos);

        if (key == "mods" && pos < text.size() && text[pos] == '[') {
            ++pos; // skip '['
            skipWhitespace(text, pos);

            while (pos < text.size() && text[pos] != ']') {
                if (text[pos] == '{') {
                    ++pos;
                    InstalledModInfo info;

                    while (pos < text.size() && text[pos] != '}') {
                        skipWhitespace(text, pos);
                        if (text[pos] != '"') { ++pos; continue; }

                        std::string k = readJsonString(text, pos);
                        skipWhitespace(text, pos);
                        if (pos >= text.size() || text[pos] != ':') break;
                        ++pos;
                        skipWhitespace(text, pos);

                        if (pos < text.size() && text[pos] == '"') {
                            std::string v = readJsonString(text, pos);
                            if (k == "name") info.name = v;
                            else if (k == "version") info.version = v;
                            else if (k == "author") info.author = v;
                            else if (k == "game") info.game = v;
                            else if (k == "installedAt") info.installedAt = v;
                            else if (k == "zipPath") info.zipPath = v;
                        } else {
                            while (pos < text.size() && text[pos] != ',' && text[pos] != '}') ++pos;
                        }

                        skipWhitespace(text, pos);
                        if (pos < text.size() && text[pos] == ',') ++pos;
                    }

                    if (pos < text.size()) ++pos; // skip '}'
                    if (!info.name.empty()) {
                        result.push_back(std::move(info));
                    }
                } else {
                    ++pos;
                }
                skipWhitespace(text, pos);
                if (pos < text.size() && text[pos] == ',') ++pos;
                skipWhitespace(text, pos);
            }

            if (pos < text.size()) ++pos; // skip ']'
        } else {
            while (pos < text.size() && text[pos] != ',' && text[pos] != '}') ++pos;
        }

        skipWhitespace(text, pos);
        if (pos < text.size() && text[pos] == ',') ++pos;
    }

    return result;
}

std::string serializeInstalledModsJson(const std::vector<InstalledModInfo>& mods) {
    std::ostringstream out;
    out << "{\n  \"mods\": [\n";
    for (size_t i = 0; i < mods.size(); ++i) {
        const auto& m = mods[i];
        out << "    {\n";
        out << "      \"name\": \"" << escapeJsonString(m.name) << "\",\n";
        out << "      \"version\": \"" << escapeJsonString(m.version) << "\",\n";
        out << "      \"author\": \"" << escapeJsonString(m.author) << "\",\n";
        out << "      \"game\": \"" << escapeJsonString(m.game) << "\",\n";
        out << "      \"installedAt\": \"" << escapeJsonString(m.installedAt) << "\",\n";
        out << "      \"zipPath\": \"" << escapeJsonString(m.zipPath) << "\"\n";
        out << "    }";
        if (i + 1 < mods.size()) out << ",";
        out << "\n";
    }
    out << "  ]\n}\n";
    return out.str();
}

} // namespace kuf
