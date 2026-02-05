#include "core/recent_files.h"

#include <algorithm>

namespace kuf {

RecentFiles::RecentFiles(size_t maxFiles) : maxFiles_(maxFiles) {}

void RecentFiles::add(const std::string& path) {
    auto it = std::find(files_.begin(), files_.end(), path);
    if (it != files_.end()) {
        files_.erase(it);
    }

    files_.insert(files_.begin(), path);

    if (files_.size() > maxFiles_) {
        files_.resize(maxFiles_);
    }
}

void RecentFiles::remove(const std::string& path) {
    auto it = std::find(files_.begin(), files_.end(), path);
    if (it != files_.end()) {
        files_.erase(it);
    }
}

void RecentFiles::clear() {
    files_.clear();
}

void RecentFiles::setMaxFiles(size_t max) {
    maxFiles_ = max;
    if (files_.size() > maxFiles_) {
        files_.resize(maxFiles_);
    }
}

} // namespace kuf
