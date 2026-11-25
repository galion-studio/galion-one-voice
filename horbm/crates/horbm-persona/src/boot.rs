// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Which persona id development boot may install.
//!
//! The id comes from [`BootConfig::ENV`] or from `development.toml`.
//! Nothing in this module names a persona. A blank source is an error.

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

    /// `development_persona = "<id>"`. A missing or blank id is not configured.
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
