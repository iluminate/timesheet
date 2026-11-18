#![forbid(unsafe_code)]

mod api;
mod browser;
mod commands;
mod config;
mod dates;
mod error;
mod log;
mod login;
mod model;
mod ui;

use std::io::IsTerminal;
use std::process::ExitCode;

use error::{AppError, Result};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTO_LOGIN_TIMEOUT_MIN: u64 = 10;

const USAGE: &str = "uso: timesheet [comando] [opciones]

  timesheet              estado del periodo actual y pendientes
  timesheet list         periodos recientes
  timesheet fill         rellena un periodo pendiente
  timesheet submit       envía un periodo a aprobación
  timesheet login        abre iEngage y guarda la sesión
  timesheet init         configuración inicial

Opciones globales:
  -h, --help             muestra esta ayuda
  -V, --version          muestra la versión
  -v, --verbose          muestra el detalle
  --no-login             no abre el navegador automáticamente

Cada comando acepta -h para ver su ayuda.";

fn main() -> ExitCode {
    let mut verbose = false;
    let mut no_login = false;
    let mut args = Vec::new();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-v" | "--verbose" => verbose = true,
            "--no-login" => no_login = true,
            _ => args.push(arg),
        }
    }
    ui::set_verbose(verbose);

    match run(&args, no_login) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            ui::fail(&err.message());
            ExitCode::from(err.code())
        }
    }
}

fn run(args: &[String], no_login: bool) -> Result<()> {
    match run_once(args) {
        Err(err) if should_auto_login(&err, args, no_login) => {
            ui::say("→ No hay sesión activa.");
            let config = config::Config::load()?;
            browser::login(&config, None, AUTO_LOGIN_TIMEOUT_MIN)?;
            run_once(args)
        }
        other => other,
    }
}

fn should_auto_login(err: &AppError, args: &[String], no_login: bool) -> bool {
    if no_login
        || std::env::var_os("TIMESHEET_NO_AUTO_LOGIN").is_some()
        || !std::io::stdin().is_terminal()
    {
        return false;
    }

    let command = args.first().map(String::as_str).unwrap_or("status");
    if matches!(command, "login" | "init") {
        return false;
    }

    matches!(err, AppError::Auth(_) | AppError::Session(_))
}

fn run_once(args: &[String]) -> Result<()> {
    let Some(first) = args.first() else {
        return commands::status(&[]);
    };
    match first.as_str() {
        "-h" | "--help" => {
            println!("{USAGE}");
            Ok(())
        }
        "-V" | "--version" => {
            println!("timesheet {VERSION}");
            Ok(())
        }
        "status" => commands::status(&args[1..]),
        "list" => commands::list(&args[1..]),
        "fill" => commands::run_fill(&args[1..]),
        "submit" => commands::run_submit(&args[1..]),
        "login" => commands::run_login(&args[1..]),
        "init" => commands::run_init(&args[1..]),
        other if other.starts_with('-') => commands::status(args),
        other => Err(error::msg(format!(
            "comando desconocido: {other}\n\n{USAGE}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_errors_use_exit_code_2() {
        let error = AppError::Auth("cookie no encontrada".into());
        assert_eq!(error.code(), 2);
        assert!(error.message().contains("falta iniciar sesión"));
    }
}
