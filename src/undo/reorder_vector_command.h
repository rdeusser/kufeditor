#pragma once

#include "undo/command.h"

#include <algorithm>
#include <string>
#include <utility>
#include <vector>

namespace kuf {

template<typename T>
class ReorderVectorCommand : public ICommand {
public:
    ReorderVectorCommand(std::vector<T>* vec, int srcIndex, int dstIndex, std::string desc)
        : vec_(vec)
        , srcIndex_(srcIndex)
        , dstIndex_(dstIndex)
        , description_(std::move(desc)) {}

    void execute() override {
        moveElement(srcIndex_, dstIndex_);
    }

    void undo() override {
        moveElement(dstIndex_, srcIndex_);
    }

    std::string description() const override {
        return description_;
    }

private:
    void moveElement(int from, int to) {
        auto item = std::move((*vec_)[from]);
        vec_->erase(vec_->begin() + from);
        vec_->insert(vec_->begin() + to, std::move(item));
    }

    std::vector<T>* vec_;
    int srcIndex_;
    int dstIndex_;
    std::string description_;
};

} // namespace kuf
