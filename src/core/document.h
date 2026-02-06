#pragma once

#include "formats/sox_binary.h"
#include "formats/sox_text.h"
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
    std::shared_ptr<SoxText> textData;
    std::vector<std::byte> rawData;
    bool isSoxEncoded = false;
    bool dirty = false;
    std::unique_ptr<UndoStack> undoStack;

    OpenDocument() : undoStack(std::make_unique<UndoStack>()) {}

    bool isBinary() const { return binaryData != nullptr; }
    bool isText() const { return textData != nullptr; }
    bool hasData() const { return isBinary() || isText(); }
};

} // namespace kuf
