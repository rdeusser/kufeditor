#pragma once

#include <memory>
#include <string>

namespace kuf {

class ICommand {
public:
    virtual ~ICommand() = default;

    virtual void execute() = 0;
    virtual void undo() = 0;
    virtual std::string description() const = 0;
};

using CommandPtr = std::unique_ptr<ICommand>;

} // namespace kuf
