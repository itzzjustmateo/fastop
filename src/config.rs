//! Cross-platform configuration file handling.
//!
//! The configuration is stored as `config.toml` inside the platform specific
//! Fastop configuration directory, which is resolved with the [`directories`]
//! crate:
//!
//! | Platform | Path                                                  |
//! |----------|-------------------------------------------------------|
//! | Linux    | `~/.config/fastop/config.toml`                        |
//! | macOS    | `~/Library/Application Support/fastop/config.toml`    |
//! | Windows  | `%APPDATA%\fastop\config.toml`                        |
//!
//! [`directories::BaseDirs`] is used (rather than `ProjectDirs`) because a
//! project directory would append an extra `config` component on Windows,
//! producing `%APPDATA%\fastop\config\config.toml` instead of the intended
//! `%APPDATA%\fastop\config.toml`.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::cli::{DEFAULT_TICK_MS, LayoutMode, MAX_TICK_MS, MIN_TICK_MS};

/// Application directory created inside the platform configuration directory.
const APP_DIR: &str = "fastop";

/// Name of the configuration file inside [`APP_DIR`].
const CONFIG_FILE: &str = "config.toml";

/// User-configurable settings loaded from `config.toml`.
///
/// Every field is optional in the file: missing keys fall back to
/// [`Config::default`], so a partial configuration is always valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    /// Refresh interval in milliseconds.
    pub(crate) tick: u64,
    /// Preferred arrangement of the summary panels.
    pub(crate) layout: LayoutMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tick: DEFAULT_TICK_MS,
            layout: LayoutMode::Auto,
        }
    }
}

impl Config {
    /// Refresh interval as a [`Duration`], clamped to the supported range.
    pub(crate) fn tick_rate(&self) -> Duration {
        Duration::from_millis(self.tick.clamp(MIN_TICK_MS, MAX_TICK_MS))
    }

    /// Applies command-line overrides on top of the loaded configuration.
    ///
    /// Values supplied on the command line take precedence over the
    /// configuration file, which in turn takes precedence over the built-in
    /// defaults.
    pub(crate) fn with_overrides(self, tick: Option<u64>, layout: Option<LayoutMode>) -> Self {
        Self {
            tick: tick.unwrap_or(self.tick).clamp(MIN_TICK_MS, MAX_TICK_MS),
            layout: layout.unwrap_or(self.layout),
        }
    }

    /// Resolves the platform-specific path of the configuration file.
    pub(crate) fn path() -> Result<PathBuf, ConfigError> {
        let base_dirs = BaseDirs::new().ok_or(ConfigError::NoConfigDir)?;
        Ok(base_dirs.config_dir().join(APP_DIR).join(CONFIG_FILE))
    }

    /// Loads the configuration, falling back to defaults when the file is
    /// missing.
    ///
    /// When `path` is `None` the platform specific location is used.
    pub(crate) fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        match path {
            Some(path) => Self::load_from(path),
            None => Self::load_from(&Self::path()?),
        }
    }

    /// Writes the configuration to disk, creating the directory when necessary.
    ///
    /// Keys already present in the file that are not managed by [`Config`] are
    /// preserved. Returns the path the configuration was written to.
    pub(crate) fn save(&self, path: Option<&Path>) -> Result<PathBuf, ConfigError> {
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => Self::path()?,
        };

        self.save_to(&path)?;
        Ok(path)
    }

    /// Loads from an explicit path, returning defaults when it does not exist.
    fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let contents = match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Serializes to TOML at an explicit path, preserving unmanaged keys.
    fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        // Start from the existing document so unrelated keys are not dropped.
        let mut table: toml::Table = std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| contents.parse().ok())
            .unwrap_or_default();

        let managed = toml::Table::try_from(self).map_err(|source| ConfigError::Serialize {
            path: path.to_path_buf(),
            source,
        })?;
        table.extend(managed);

        let contents = toml::to_string_pretty(&table).map_err(|source| ConfigError::Serialize {
            path: path.to_path_buf(),
            source,
        })?;

        std::fs::write(path, contents).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Errors produced while locating, reading, or writing the configuration.
#[derive(Debug)]
pub(crate) enum ConfigError {
    /// The platform configuration directory could not be determined.
    NoConfigDir,
    /// The configuration file could not be read or written.
    Io {
        /// Path that could not be accessed.
        path: PathBuf,
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// The configuration file contained invalid TOML.
    Parse {
        /// Path of the invalid file.
        path: PathBuf,
        source: toml::de::Error,
    },
    /// The configuration could not be serialized to TOML.
    Serialize {
        /// Destination path being written.
        path: PathBuf,
        source: toml::ser::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfigDir => {
                formatter.write_str("could not determine the platform configuration directory")
            }
            Self::Io { path, source } => {
                write!(formatter, "failed to access {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(
                    formatter,
                    "invalid configuration in {}: {source}",
                    path.display()
                )
            }
            Self::Serialize { path, source } => {
                write!(
                    formatter,
                    "failed to serialize configuration for {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoConfigDir => None,
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Serialize { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a unique, empty directory for a single test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fastop-config-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn defaults_when_file_is_missing() {
        let dir = temp_dir("missing");

        let config = Config::load_from(&dir.join("config.toml")).expect("load defaults");

        assert_eq!(config, Config::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parses_all_fields() {
        let dir = temp_dir("parses");
        let path = dir.join("config.toml");
        std::fs::write(&path, "tick = 250\nlayout = \"grid\"\n").unwrap();

        let config = Config::load_from(&path).expect("parse config");

        assert_eq!(config.tick, 250);
        assert_eq!(config.layout, LayoutMode::Grid);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let dir = temp_dir("partial");
        let path = dir.join("config.toml");
        std::fs::write(&path, "tick = 1000\n").unwrap();

        let config = Config::load_from(&path).expect("parse partial config");

        assert_eq!(config.tick, 1000);
        assert_eq!(config.layout, LayoutMode::Auto);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_toml_reports_an_error() {
        let dir = temp_dir("invalid");
        let path = dir.join("config.toml");
        std::fs::write(&path, "tick = \n").unwrap();

        let error = Config::load_from(&path).expect_err("invalid TOML should fail");

        assert!(matches!(error, ConfigError::Parse { .. }));
        assert!(error.to_string().contains("invalid configuration"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_creates_directory_and_round_trips() {
        let dir = temp_dir("save");
        let path = dir.join("nested").join("config.toml");
        let config = Config {
            tick: 750,
            layout: LayoutMode::Compact,
        };

        let written = config.save(Some(&path)).expect("save config");

        assert_eq!(written, path);
        assert_eq!(Config::load_from(&path).unwrap(), config);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_preserves_unmanaged_keys() {
        let dir = temp_dir("preserve");
        let path = dir.join("config.toml");
        std::fs::write(&path, "tick = 250\ntheme = \"dark\"\n").unwrap();

        Config {
            tick: 1500,
            layout: LayoutMode::Grid,
        }
        .save(Some(&path))
        .expect("save config");

        let saved: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap())
            .expect("re-parse saved config");
        assert_eq!(
            saved.get("tick").and_then(toml::Value::as_integer),
            Some(1500)
        );
        assert_eq!(
            saved.get("layout").and_then(toml::Value::as_str),
            Some("grid")
        );
        assert_eq!(
            saved.get("theme").and_then(toml::Value::as_str),
            Some("dark")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cli_overrides_take_precedence() {
        let config = Config {
            tick: 250,
            layout: LayoutMode::Compact,
        };

        let merged = config.clone().with_overrides(Some(1000), None);
        assert_eq!(merged.tick, 1000);
        assert_eq!(merged.layout, LayoutMode::Compact);

        let merged = Config::default().with_overrides(None, Some(LayoutMode::Grid));
        assert_eq!(merged.tick, DEFAULT_TICK_MS);
        assert_eq!(merged.layout, LayoutMode::Grid);
    }

    #[test]
    fn tick_is_clamped_to_supported_range() {
        let low = Config {
            tick: 1,
            layout: LayoutMode::Auto,
        };
        assert_eq!(low.tick_rate(), Duration::from_millis(MIN_TICK_MS));

        let high = Config {
            tick: u64::MAX,
            layout: LayoutMode::Auto,
        };
        assert_eq!(high.tick_rate(), Duration::from_millis(MAX_TICK_MS));
    }

    #[test]
    fn config_path_uses_fastop_directory() {
        let Ok(path) = Config::path() else {
            return;
        };

        assert_eq!(path.file_name().unwrap(), CONFIG_FILE);
        assert_eq!(path.parent().unwrap().file_name().unwrap(), APP_DIR);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_linux_config_directory() {
        let Ok(path) = Config::path() else {
            return;
        };

        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
            let xdg = Path::new(&xdg);
            if xdg.is_absolute() {
                assert!(
                    path.starts_with(xdg),
                    "{} should be under {}",
                    path.display(),
                    xdg.display()
                );
            }
        } else if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
            let expected = Path::new(&home).join(".config");
            assert!(
                path.starts_with(&expected),
                "{} should be under {}",
                path.display(),
                expected.display()
            );
        }
    }
}
