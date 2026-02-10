#pragma once

#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace kuf {

class ZipReader {
public:
    ZipReader() = default;
    ~ZipReader();

    ZipReader(const ZipReader&) = delete;
    ZipReader& operator=(const ZipReader&) = delete;
    ZipReader(ZipReader&& other) noexcept;
    ZipReader& operator=(ZipReader&& other) noexcept;

    bool open(const std::string& path);
    void close();

    std::vector<std::string> entries() const;
    std::optional<std::vector<std::byte>> readEntry(const std::string& name) const;
    bool extractEntry(const std::string& name, const std::string& destPath) const;
    bool extractAll(const std::string& destDir) const;

private:
    void* archive_ = nullptr; // mz_zip_archive*
};

class ZipWriter {
public:
    ZipWriter() = default;
    ~ZipWriter();

    ZipWriter(const ZipWriter&) = delete;
    ZipWriter& operator=(const ZipWriter&) = delete;
    ZipWriter(ZipWriter&& other) noexcept;
    ZipWriter& operator=(ZipWriter&& other) noexcept;

    bool create(const std::string& path);
    bool addFile(const std::string& diskPath, const std::string& archiveName);
    bool addMemory(const std::string& archiveName, const void* data, size_t size);
    bool finalize();

private:
    void* archive_ = nullptr; // mz_zip_archive*
    bool finalized_ = false;
};

} // namespace kuf
