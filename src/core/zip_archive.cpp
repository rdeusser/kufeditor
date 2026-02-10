#include "core/zip_archive.h"

#include <miniz.h>

#include <filesystem>
#include <fstream>

namespace kuf {

// --- ZipReader ---

ZipReader::~ZipReader() { close(); }

ZipReader::ZipReader(ZipReader&& other) noexcept : archive_(other.archive_) {
    other.archive_ = nullptr;
}

ZipReader& ZipReader::operator=(ZipReader&& other) noexcept {
    if (this != &other) {
        close();
        archive_ = other.archive_;
        other.archive_ = nullptr;
    }
    return *this;
}

bool ZipReader::open(const std::string& path) {
    close();
    auto* zip = new mz_zip_archive{};
    if (!mz_zip_reader_init_file(zip, path.c_str(), 0)) {
        delete zip;
        return false;
    }
    archive_ = zip;
    return true;
}

void ZipReader::close() {
    if (archive_) {
        auto* zip = static_cast<mz_zip_archive*>(archive_);
        mz_zip_reader_end(zip);
        delete zip;
        archive_ = nullptr;
    }
}

std::vector<std::string> ZipReader::entries() const {
    std::vector<std::string> result;
    if (!archive_) return result;
    auto* zip = static_cast<mz_zip_archive*>(archive_);
    mz_uint count = mz_zip_reader_get_num_files(zip);
    for (mz_uint i = 0; i < count; ++i) {
        char filename[512];
        mz_zip_reader_get_filename(zip, i, filename, sizeof(filename));
        if (!mz_zip_reader_is_file_a_directory(zip, i)) {
            result.emplace_back(filename);
        }
    }
    return result;
}

std::optional<std::vector<std::byte>> ZipReader::readEntry(const std::string& name) const {
    if (!archive_) return std::nullopt;
    auto* zip = static_cast<mz_zip_archive*>(archive_);
    int index = mz_zip_reader_locate_file(zip, name.c_str(), nullptr, 0);
    if (index < 0) return std::nullopt;

    mz_zip_archive_file_stat stat;
    if (!mz_zip_reader_file_stat(zip, static_cast<mz_uint>(index), &stat)) return std::nullopt;

    std::vector<std::byte> data(stat.m_uncomp_size);
    if (!mz_zip_reader_extract_to_mem(zip, static_cast<mz_uint>(index), data.data(), data.size(), 0)) {
        return std::nullopt;
    }
    return data;
}

bool ZipReader::extractEntry(const std::string& name, const std::string& destPath) const {
    if (!archive_) return false;
    auto* zip = static_cast<mz_zip_archive*>(archive_);
    int index = mz_zip_reader_locate_file(zip, name.c_str(), nullptr, 0);
    if (index < 0) return false;

    std::filesystem::create_directories(std::filesystem::path(destPath).parent_path());
    return mz_zip_reader_extract_to_file(zip, static_cast<mz_uint>(index), destPath.c_str(), 0);
}

bool ZipReader::extractAll(const std::string& destDir) const {
    if (!archive_) return false;
    for (const auto& entry : entries()) {
        std::string destPath = destDir + "/" + entry;
        if (!extractEntry(entry, destPath)) return false;
    }
    return true;
}

// --- ZipWriter ---

ZipWriter::~ZipWriter() {
    if (archive_) {
        auto* zip = static_cast<mz_zip_archive*>(archive_);
        if (!finalized_) {
            mz_zip_writer_finalize_archive(zip);
        }
        mz_zip_writer_end(zip);
        delete zip;
        archive_ = nullptr;
    }
}

ZipWriter::ZipWriter(ZipWriter&& other) noexcept
    : archive_(other.archive_), finalized_(other.finalized_) {
    other.archive_ = nullptr;
    other.finalized_ = false;
}

ZipWriter& ZipWriter::operator=(ZipWriter&& other) noexcept {
    if (this != &other) {
        if (archive_) {
            auto* zip = static_cast<mz_zip_archive*>(archive_);
            if (!finalized_) mz_zip_writer_finalize_archive(zip);
            mz_zip_writer_end(zip);
            delete zip;
        }
        archive_ = other.archive_;
        finalized_ = other.finalized_;
        other.archive_ = nullptr;
        other.finalized_ = false;
    }
    return *this;
}

bool ZipWriter::create(const std::string& path) {
    auto* zip = new mz_zip_archive{};
    if (!mz_zip_writer_init_file(zip, path.c_str(), 0)) {
        delete zip;
        return false;
    }
    archive_ = zip;
    finalized_ = false;
    return true;
}

bool ZipWriter::addFile(const std::string& diskPath, const std::string& archiveName) {
    if (!archive_ || finalized_) return false;
    auto* zip = static_cast<mz_zip_archive*>(archive_);

    std::ifstream file(diskPath, std::ios::binary | std::ios::ate);
    if (!file) return false;
    auto size = file.tellg();
    file.seekg(0);

    std::vector<char> data(static_cast<size_t>(size));
    file.read(data.data(), size);

    return mz_zip_writer_add_mem(zip, archiveName.c_str(), data.data(),
                                  data.size(), MZ_DEFAULT_COMPRESSION);
}

bool ZipWriter::addMemory(const std::string& archiveName, const void* data, size_t size) {
    if (!archive_ || finalized_) return false;
    auto* zip = static_cast<mz_zip_archive*>(archive_);
    return mz_zip_writer_add_mem(zip, archiveName.c_str(), data, size, MZ_DEFAULT_COMPRESSION);
}

bool ZipWriter::finalize() {
    if (!archive_ || finalized_) return false;
    auto* zip = static_cast<mz_zip_archive*>(archive_);
    finalized_ = true;
    return mz_zip_writer_finalize_archive(zip);
}

} // namespace kuf
