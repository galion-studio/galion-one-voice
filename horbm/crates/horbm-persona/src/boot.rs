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
const BAD_ID: &str = "development persona id is not a short token";
const BAD_FILE: &str = "development persona file does not match the schema";

/// The id a development boot may install. Copy. No default name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootConfig {
    id: [u8; TOKEN_BYTES],
}

impl BootConfig {
    /// Process environment that names the persona id. Wins over the file.
    pub const ENV: &str = "GALION_HORBM_DEV_PERSONA";

    /// Directory that holds `development.toml` and `personas/<id>.toml`.
    /// A non-empty value replaces this crate's directory. Blank is ignored.
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
        let raw: RawBoot = toml::from_str(text).map_err(|_| PersonaError::Invalid(BAD_FILE))?;
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

    /// [`Self::DIR`] when it is a non-empty path, otherwise this crate.
    pub fn root() -> PathBuf {
        match std::env::var(Self::DIR) {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
            _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        }
    }

    pub fn id(&self) -> &str {
        as_str(&self.id)
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoot {
    development_persona: Option<String>,
}
