#pragma once

#include "undo/command.h"

#include <functional>
#include <vector>

namespace kuf {

class UndoStack {
public:
    void execute(CommandPtr cmd);
    void undo();
    void redo();

    bool canUndo() const { return !undoStack_.empty(); }
    bool canRedo() const { return !redoStack_.empty(); }

    const std::string& undoDescription() const;
    const std::string& redoDescription() const;

    void clear();

    void setOnChange(std::function<void()> callback) { onChange_ = std::move(callback); }

private:
    void notifyChange();

    std::vector<CommandPtr> undoStack_;
    std::vector<CommandPtr> redoStack_;
    std::function<void()> onChange_;

    static const std::string emptyStr_;
};

} // namespace kuf
