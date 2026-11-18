use std::fs::OpenOptions;
use std::io::Write;

use chrono::Local;

use crate::{config, ui};

fn stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn write(level: &str, message: &str) {
    let Ok(path) = config::log_path() else {
        return;
    };
    let _ = config::ensure_dir();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {level} {message}", stamp());
    }
}

/// Detalle de depuración: archivo siempre, consola solo con -v.
pub fn line(message: &str) {
    write("INFO", message);
    if ui::verbose() {
        eprintln!("{} {message}", ui::dim(&stamp()));
    }
}

/// Aviso visible siempre.
pub fn notice(message: &str) {
    write("WARN", message);
    eprintln!("{} {message}", ui::yellow("!"));
}
