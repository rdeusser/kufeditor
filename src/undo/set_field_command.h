#pragma once

#include "undo/command.h"

#include <string>
#include <utility>

namespace kuf {

// Generic command for setting any field value.
template<typename T>
class SetFieldCommand : public ICommand {
public:
    SetFieldCommand(T* field, T newValue, std::string desc)
        : field_(field)
        , oldValue_(*field)
        , newValue_(std::move(newValue))
        , description_(std::move(desc)) {}

    void execute() override {
        *field_ = newValue_;
    }

    void undo() override {
        *field_ = oldValue_;
    }

    std::string description() const override {
        return description_;
    }

private:
    T* field_;
    T oldValue_;
    T newValue_;
    std::string description_;
};

template<typename T>
CommandPtr makeSetFieldCommand(T* field, T newValue, std::string desc) {
    return std::make_unique<SetFieldCommand<T>>(field, std::move(newValue), std::move(desc));
}

} // namespace kuf
