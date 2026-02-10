#include "undo/undo_stack.h"

namespace kuf {

void UndoStack::execute(CommandPtr cmd) {
    cmd->execute();
    undoStack_.push_back(std::move(cmd));
    redoStack_.clear();
    notifyChange();
}

void UndoStack::undo() {
    if (undoStack_.empty()) return;

    auto cmd = std::move(undoStack_.back());
    undoStack_.pop_back();
    cmd->undo();
    redoStack_.push_back(std::move(cmd));
    notifyChange();
}

void UndoStack::redo() {
    if (redoStack_.empty()) return;

    auto cmd = std::move(redoStack_.back());
    redoStack_.pop_back();
    cmd->execute();
    undoStack_.push_back(std::move(cmd));
    notifyChange();
}

std::string UndoStack::undoDescription() const {
    if (undoStack_.empty()) return {};
    return undoStack_.back()->description();
}

std::string UndoStack::redoDescription() const {
    if (redoStack_.empty()) return {};
    return redoStack_.back()->description();
}

void UndoStack::clear() {
    undoStack_.clear();
    redoStack_.clear();
    notifyChange();
}

void UndoStack::notifyChange() {
    if (onChange_) {
        onChange_();
    }
}

} // namespace kuf
