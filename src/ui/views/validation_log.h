#pragma once

#include "ui/views/view.h"
#include "formats/validation.h"

#include <functional>
#include <vector>

namespace kuf {

class ValidationLogView : public View {
public:
    ValidationLogView();

    void drawContent() override;

    void setIssues(std::vector<ValidationIssue> issues);
    void clear();

    // Callback when user clicks an issue to navigate to it.
    void setOnNavigate(std::function<void(size_t recordIndex)> callback) {
        onNavigate_ = std::move(callback);
    }

private:
    const char* severityIcon(Severity severity) const;
    ImVec4 severityColor(Severity severity) const;

    std::vector<ValidationIssue> issues_;
    std::function<void(size_t)> onNavigate_;
};

} // namespace kuf
