#pragma once

#include <string>
#include <vector>

namespace kuf {

// Manages a list of recently opened files.
class RecentFiles {
public:
    explicit RecentFiles(size_t maxFiles = 10);

    // Adds a file to the recent list (moves to front if already present).
    void add(const std::string& path);

    // Removes a file from the recent list.
    void remove(const std::string& path);

    // Clears all recent files.
    void clear();

    // Returns the list of recent files (most recent first).
    const std::vector<std::string>& files() const { return files_; }
    std::vector<std::string>& files() { return files_; }

    // Returns true if the list is empty.
    bool empty() const { return files_.empty(); }

    // Sets the maximum number of files to keep.
    void setMaxFiles(size_t max);

private:
    std::vector<std::string> files_;
    size_t maxFiles_;
};

} // namespace kuf
