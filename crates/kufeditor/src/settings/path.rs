use std::{env, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "the pure path tests construct platforms not compiled on this host"
)]
enum SettingsPlatform {
    MacOs,
    Windows,
    Unix,
}

#[derive(Default)]
struct SettingsDirectories {
    home: Option<PathBuf>,
    app_data: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
}

pub(crate) fn settings_path() -> PathBuf {
    let directories = SettingsDirectories {
        home: env::var_os("HOME").map(PathBuf::from),
        app_data: env::var_os("APPDATA").map(PathBuf::from),
        xdg_config_home: env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
    };

    #[cfg(target_os = "macos")]
    let platform = SettingsPlatform::MacOs;
    #[cfg(target_os = "windows")]
    let platform = SettingsPlatform::Windows;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let platform = SettingsPlatform::Unix;

    settings_path_for(platform, &directories)
}

fn settings_path_for(platform: SettingsPlatform, directories: &SettingsDirectories) -> PathBuf {
    let base = match platform {
        SettingsPlatform::MacOs => directories
            .home
            .as_deref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|home| home.join("Library/Application Support")),
        SettingsPlatform::Windows => directories
            .app_data
            .as_deref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(PathBuf::from),
        SettingsPlatform::Unix => directories
            .xdg_config_home
            .as_deref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                directories
                    .home
                    .as_deref()
                    .filter(|path| !path.as_os_str().is_empty())
                    .map(|home| home.join(".config"))
            }),
    };

    base.map_or_else(
        || PathBuf::from("settings.json"),
        |base| base.join("kufeditor/settings.json"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{SettingsDirectories, SettingsPlatform, settings_path_for};

    #[test]
    fn macos_uses_application_support_below_home() {
        let directories = SettingsDirectories {
            home: Some(PathBuf::from("/Users/alice")),
            ..SettingsDirectories::default()
        };

        assert_eq!(
            settings_path_for(SettingsPlatform::MacOs, &directories),
            PathBuf::from("/Users/alice/Library/Application Support/kufeditor/settings.json")
        );
    }

    #[test]
    fn windows_uses_roaming_application_data() {
        let directories = SettingsDirectories {
            app_data: Some(PathBuf::from(r"C:\Users\alice\AppData\Roaming")),
            ..SettingsDirectories::default()
        };

        assert_eq!(
            settings_path_for(SettingsPlatform::Windows, &directories),
            PathBuf::from(r"C:\Users\alice\AppData\Roaming")
                .join("kufeditor")
                .join("settings.json")
        );
    }

    #[test]
    fn unix_prefers_xdg_config_home() {
        let directories = SettingsDirectories {
            home: Some(PathBuf::from("/home/alice")),
            xdg_config_home: Some(PathBuf::from("/var/config/alice")),
            ..SettingsDirectories::default()
        };

        assert_eq!(
            settings_path_for(SettingsPlatform::Unix, &directories),
            PathBuf::from("/var/config/alice/kufeditor/settings.json")
        );
    }

    #[test]
    fn unix_falls_back_to_dot_config_below_home() {
        let directories = SettingsDirectories {
            home: Some(PathBuf::from("/home/alice")),
            ..SettingsDirectories::default()
        };

        assert_eq!(
            settings_path_for(SettingsPlatform::Unix, &directories),
            PathBuf::from("/home/alice/.config/kufeditor/settings.json")
        );
    }

    #[test]
    fn missing_platform_directories_use_a_relative_file() {
        let directories = SettingsDirectories::default();

        for platform in [
            SettingsPlatform::MacOs,
            SettingsPlatform::Windows,
            SettingsPlatform::Unix,
        ] {
            assert_eq!(
                settings_path_for(platform, &directories),
                PathBuf::from("settings.json")
            );
        }
    }
}
