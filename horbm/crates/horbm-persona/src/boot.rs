// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Which persona id development boot may install.
//!
//! The id comes from [`BootConfig::ENV`] or from `development.toml`.
//! Nothing in this module names a persona. A blank source is an error.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::load::pack_name;
use crate::{as_str, PersonaError, TOKEN_BYTES};

const SHIPPED_BOOT_TOML: &str = include_str!("../development.toml");
const UNSET: &str = "development persona is not configured";
const NO_DIR: &str = "development persona directory is not configured";
const ABS_DIR: &str = "persona directory must be a relative path";
const BAD_ID: &str = "development persona id is not a short token";
const BAD_FILE: &str = "development persona file does not match the schema";
const BOOT_NAME: &str = "development.toml";
/// Relative places a working directory may keep the boot file.
/// None of these is an absolute path compiled into the binary.
const BOOT_REL: &[&str] = &[
    BOOT_NAME,
    "horbm-persona/development.toml",
    "crates/horbm-persona/development.toml",
    "horbm/crates/horbm-persona/development.toml",
];

/// The id a development boot may install. Copy. No default name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootConfig {
    id: [u8; TOKEN_BYTES],
}

impl BootConfig {
    /// Process environment that names the persona id. Wins over the file.
    pub const ENV: &str = "GALION_HORBM_DEV_PERSONA";

    /// Directory of `<id>.toml` files. Wins over [`Self::files_from_toml`].
    /// Blank is ignored.
    pub const DIR: &str = "GALION_HORBM_PERSONA_DIR";

    /// One persona id. Blank is not configured. Any other illegal token is refused.
    pub fn from_id(id: &str) -> Result<Self, PersonaError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(PersonaError::Invalid(UNSET));
        }
        Ok(Self {
            id: pack_name(id, BAD_ID)?,
        })
    }

    /// The `development_persona` key. A missing or blank id is not configured.
    pub fn from_toml(text: &str) -> Result<Self, PersonaError> {
        let raw = raw_boot(text)?;
        match raw.development_persona {
            Some(id) => Self::from_id(&id),
            None => Err(PersonaError::Invalid(UNSET)),
        }
    }

    /// A non-empty environment value wins. Otherwise the TOML text.
    /// Both absent, or a present value that is not an id, is an error.
    pub fn resolve(env_value: Option<&str>, toml_text: Option<&str>) -> Result<Self, PersonaError> {
        if let Some(id) = nonempty(env_value) {
            return Self::from_id(id);
        }
        match nonempty(toml_text) {
            Some(text) => Self::from_toml(text),
            None => Err(PersonaError::Invalid(UNSET)),
        }
    }

    /// The repo file, ignoring the process environment.
    pub fn shipped() -> Result<Self, PersonaError> {
        Self::from_toml(SHIPPED_BOOT_TOML)
    }

    /// [`Self::ENV`] when it is set to an id, otherwise the repo file.
    pub fn from_process() -> Result<Self, PersonaError> {
        let env = std::env::var(Self::ENV).ok();
        Self::resolve(env.as_deref(), Some(SHIPPED_BOOT_TOML))
    }

    /// `env_value` wins when it is a non-empty id. Otherwise
    /// `root/development.toml`. A missing file is not configured.
    /// Any other read failure is [`PersonaError::Unreadable`].
    pub fn from_dir(root: &Path, env_value: Option<&str>) -> Result<Self, PersonaError> {
        if nonempty(env_value).is_some() {
            return Self::resolve(env_value, None);
        }
        let path = root.join("development.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::from_toml(&text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(PersonaError::Invalid(UNSET))
            }
            Err(_) => Err(PersonaError::Unreadable),
        }
    }

    /// Directory of `<id>.toml` files.
    ///
    /// [`Self::DIR`] wins when it is non-empty. Otherwise `persona_dir` in
    /// the boot file is joined to that file's directory. If neither source
    /// is present, that is an error.
    pub fn resolve_root(
        env_dir: Option<&Path>,
        toml_path: Option<&Path>,
    ) -> Result<PathBuf, PersonaError> {
        if let Some(dir) = env_dir {
            return Ok(dir.to_path_buf());
        }
        match toml_path {
            Some(path) => Self::files_from_toml(path),
            None => Err(PersonaError::Invalid(NO_DIR)),
        }
    }

    /// `persona_dir` in `path`, relative to the directory that holds `path`.
    pub fn files_from_toml(path: &Path) -> Result<PathBuf, PersonaError> {
        let text = std::fs::read_to_string(path).map_err(|_| PersonaError::Unreadable)?;
        let raw = raw_boot(&text)?;
        let rel = raw
            .persona_dir
            .as_deref()
            .ok_or(PersonaError::Invalid(NO_DIR))?;
        let rel = relative_dir(rel)?;
        let base = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Ok(base.join(rel))
    }

    /// [`Self::DIR`], otherwise the boot file found from the working directory.
    pub fn root() -> Result<PathBuf, PersonaError> {
        if let Some(dir) = dir_from_env() {
            return Ok(dir);
        }
        Self::files_from_toml(&locate_boot_file()?)
    }

    /// Boot text next to a persona-file directory, or in its parent.
    /// A missing file is `None`. The file in `files` is returned even when
    /// it does not match the schema. A parent file is used only when it does.
    pub(crate) fn boot_text_near(files: &Path) -> Result<Option<String>, PersonaError> {
        if let Some(text) = read_boot(&files.join(BOOT_NAME))? {
            return Ok(Some(text));
        }
        if let Some(parent) = files.parent() {
            if parent != files {
                if let Some(text) = read_boot(&parent.join(BOOT_NAME))? {
                    if raw_boot(&text).is_ok() {
                        return Ok(Some(text));
                    }
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn env_dir() -> Option<PathBuf> {
        dir_from_env()
    }

    pub(crate) fn locate() -> Result<PathBuf, PersonaError> {
        locate_boot_file()
    }

    pub fn id(&self) -> &str {
        as_str(&self.id)
    }
}

fn dir_from_env() -> Option<PathBuf> {
    let raw = std::env::var(BootConfig::DIR).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn raw_boot(text: &str) -> Result<RawBoot, PersonaError> {
    toml::from_str(text).map_err(|_| PersonaError::Invalid(BAD_FILE))
}

fn relative_dir(value: &str) -> Result<&str, PersonaError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(PersonaError::Invalid(NO_DIR));
    }
    if Path::new(value).is_absolute() {
        return Err(PersonaError::Invalid(ABS_DIR));
    }
    Ok(value)
}

fn read_boot(path: &Path) -> Result<Option<String>, PersonaError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(PersonaError::Unreadable),
    }
}

fn locate_boot_file() -> Result<PathBuf, PersonaError> {
    let mut dir = std::env::current_dir().map_err(|_| PersonaError::Unreadable)?;
    for _ in 0..32 {
        for rel in BOOT_REL {
            let path = dir.join(rel);
            if path.is_file() && boot_file_ok(&path) {
                return Ok(path);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    Err(PersonaError::Invalid(NO_DIR))
}

fn boot_file_ok(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    matches!(
        raw_boot(&text),
        Ok(raw) if raw.development_persona.is_some() || raw.persona_dir.is_some()
    )
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoot {
    development_persona: Option<String>,
    persona_dir: Option<String>,
}
