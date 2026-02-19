#pragma once

#include <string>
#include <vector>

namespace kuf {

struct SteamGame {
    std::string name;
    std::string soxPath;
};

/// Scans all drives for known KUF game installations under common Steam
/// library locations. Returns one entry per discovered game with the path
/// to its Data/SOX directory.
std::vector<SteamGame> detectSteamGames();

/// Finds the Data/SOX directory for KUF Crusader by scanning all drives.
/// Returns an empty string if not found.
std::string findSteamSoxDirectory();

} // namespace kuf
