#include "ui/dialogs/file_dialog.h"

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commdlg.h>
#include <shlobj.h>
#endif

#ifdef __APPLE__
// macOS implementation uses Objective-C, defined in file_dialog_macos.mm.
extern std::optional<std::string> macosOpenFile(const char* filter, const char* initialDir);
extern std::optional<std::string> macosSaveFile(const char* filter, const char* defaultName);
extern std::optional<std::string> macosOpenFolder();
#endif

// TODO: Add Linux file dialog support.

namespace kuf {

std::optional<std::string> FileDialog::openFile(const char* filter, const char* initialDir) {
#ifdef _WIN32
    char filename[MAX_PATH] = "";

    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrInitialDir = initialDir;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;

    if (GetOpenFileNameA(&ofn)) {
        return std::string(filename);
    }
#elif defined(__APPLE__)
    return macosOpenFile(filter, initialDir);
#endif
    return std::nullopt;
}

std::optional<std::string> FileDialog::saveFile(const char* filter, const char* defaultName) {
#ifdef _WIN32
    char filename[MAX_PATH] = "";
    if (defaultName) {
        strncpy_s(filename, defaultName, MAX_PATH - 1);
    }

    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;

    if (GetSaveFileNameA(&ofn)) {
        return std::string(filename);
    }
#elif defined(__APPLE__)
    return macosSaveFile(filter, defaultName);
#endif
    return std::nullopt;
}

std::optional<std::string> FileDialog::openFolder() {
#ifdef _WIN32
    char path[MAX_PATH] = "";

    BROWSEINFOA bi = {};
    bi.lpszTitle = "Select Game SOX Folder";
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;

    LPITEMIDLIST pidl = SHBrowseForFolderA(&bi);
    if (pidl) {
        if (SHGetPathFromIDListA(pidl, path)) {
            CoTaskMemFree(pidl);
            return std::string(path);
        }
        CoTaskMemFree(pidl);
    }
#elif defined(__APPLE__)
    return macosOpenFolder();
#endif
    return std::nullopt;
}

} // namespace kuf
