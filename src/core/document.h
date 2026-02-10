#pragma once

#include "formats/sox_binary.h"
#include "formats/sox_skill_info.h"
#include "formats/sox_text.h"
#include "formats/stg_format.h"
#include "undo/undo_stack.h"

#include <memory>
#include <string>
#include <vector>

namespace kuf {

/// Represents an open file in the editor.
struct OpenDocument {
    std::string path;
    std::string filename;
    std::shared_ptr<SoxBinary> binaryData;
    std::shared_ptr<SoxSkillInfo> skillData;
    std::shared_ptr<SoxText> textData;
    std::shared_ptr<StgFormat> stgData;
    std::vector<std::byte> rawData;
    bool isSoxEncoded = false;
    bool dirty = false;
    std::unique_ptr<UndoStack> undoStack;

    OpenDocument() : undoStack(std::make_unique<UndoStack>()) {}

    bool isBinary() const { return binaryData != nullptr; }
    bool isSkill() const { return skillData != nullptr; }
    bool isText() const { return textData != nullptr; }
    bool isStg() const { return stgData != nullptr; }
    bool hasData() const { return isBinary() || isSkill() || isText() || isStg(); }
};

} // namespace kuf
