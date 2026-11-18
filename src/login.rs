use std::io::{self, IsTerminal, Read};

use crate::api::{self, Api};
use crate::config::{self, Config};
use crate::error::{msg, Result};
use crate::ui;

/// Respaldo avanzado para servidores sin entorno gráfico o CI.
pub enum CookieSource {
    Stdin,
    File(String),
}

pub fn from_cookie(config: &Config, source: CookieSource) -> Result<()> {
    match source {
        CookieSource::Stdin => read_stdin(config),
        CookieSource::File(path) => read_file(config, &path),
    }
}

fn read_stdin(config: &Config) -> Result<()> {
    if io::stdin().is_terminal() {
        ui::say(&ui::dim(
            "pega el header Cookie desde DevTools → Network y termina con Ctrl-D",
        ));
    }
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;
    save_raw(config, &raw)
}

fn read_file(config: &Config, path: &str) -> Result<()> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| msg(format!("no se pudo leer {path}: {err}")))?;
    save_raw(config, &raw)
}

fn save_raw(config: &Config, raw: &str) -> Result<()> {
    let cookie = api::parse_cookie(raw).ok_or_else(|| msg("cookie vacía"))?;
    let api = Api::new(config, cookie.clone());
    api.verify()?;
    config::save_cookie(&cookie)?;
    ui::ok("sesión guardada");
    Ok(())
}
