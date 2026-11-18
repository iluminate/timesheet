use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{msg, Result};

pub const DEFAULT_BASE: &str = "https://iengage.coforge.com/ess2/Timesheet/TimesheetHandler";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub emp_code: String,
    #[serde(default)]
    pub project_code: String,
    #[serde(default = "default_hours")]
    pub hours_per_day: f64,
    #[serde(default = "default_one")]
    pub activity_code: String,
    #[serde(default = "default_one")]
    pub category_code: String,
    #[serde(default = "default_zero")]
    pub shift_code: String,
    #[serde(default = "default_base")]
    pub base_url: String,
}

fn default_hours() -> f64 {
    8.0
}
fn default_one() -> String {
    "1".into()
}
fn default_zero() -> String {
    "0".into()
}
fn default_base() -> String {
    DEFAULT_BASE.into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            emp_code: String::new(),
            project_code: String::new(),
            hours_per_day: default_hours(),
            activity_code: default_one(),
            category_code: default_one(),
            shift_code: default_zero(),
            base_url: default_base(),
        }
    }
}

pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| msg("falta la variable HOME"))
}

pub fn dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("TIMESHEET_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(home()?.join(".config").join("timesheet"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(dir()?.join("config.json"))
}

pub fn cookie_path() -> Result<PathBuf> {
    Ok(dir()?.join("cookie.txt"))
}

pub fn log_path() -> Result<PathBuf> {
    Ok(dir()?.join("timesheet.log"))
}

pub fn ensure_dir() -> Result<()> {
    let dir = dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn save_cookie(cookie: &str) -> Result<()> {
    let path = cookie_path()?;
    let tmp = path.with_extension("txt.tmp");
    write_private(&tmp, &format!("{cookie}\n"))?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn write_private(path: &PathBuf, data: &str) -> Result<()> {
    ensure_dir()?;
    fs::write(path, data)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

impl Config {
    pub fn exists() -> bool {
        config_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Err(msg("falta configuración · timesheet init"));
        }
        let raw = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = config_path()?;
        write_private(&path, &format!("{}\n", serde_json::to_string_pretty(self)?))?;
        Ok(path)
    }
}
