use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{msg, Result};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(value: bool) {
    VERBOSE.store(value, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

fn color_enabled() -> bool {
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

pub fn paint(code: &str, text: &str) -> String {
    if color_enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn bold(text: &str) -> String {
    paint("1", text)
}
pub fn dim(text: &str) -> String {
    paint("2", text)
}
pub fn red(text: &str) -> String {
    paint("31", text)
}
pub fn green(text: &str) -> String {
    paint("32", text)
}
pub fn yellow(text: &str) -> String {
    paint("33", text)
}
pub fn cyan(text: &str) -> String {
    paint("36", text)
}

pub fn say(line: &str) {
    println!("{line}");
}

pub fn ok(message: &str) {
    println!("{} {message}", green("✔"));
}

pub fn warn(message: &str) {
    println!("{} {message}", yellow("!"));
}

pub fn fail(message: &str) {
    eprintln!("{} {}", red("✘"), message);
}

pub fn hint(command: &str) {
    println!("→ {}", cyan(command));
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn days_text(n: usize) -> String {
    plural(n, "día", "días")
}

/// "8h", "7.5h"
pub fn hours(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}h")
}

fn visible_len(text: &str) -> usize {
    let mut len = 0;
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
        } else {
            len += 1;
        }
    }
    len
}

fn pad(text: &str, width: usize, right: bool) -> String {
    let fill = " ".repeat(width.saturating_sub(visible_len(text)));
    if right {
        format!("{fill}{text}")
    } else {
        format!("{text}{fill}")
    }
}

pub fn table(headers: &[&str], rows: &[Vec<String>], right: &[usize]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| visible_len(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(visible_len(cell));
            }
        }
    }

    let render = |row: &[String], color: bool| {
        let mut parts = Vec::new();
        for (i, cell) in row.iter().enumerate() {
            let text = pad(cell, widths[i], right.contains(&i));
            parts.push(if color { dim(&text) } else { text });
        }
        parts.join("  ").trim_end().to_string()
    };

    let headers: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    println!("{}", render(&headers, true));
    for row in rows {
        println!("{}", render(row, false));
    }
}

pub fn select(title: &str, items: &[String]) -> Result<Option<usize>> {
    if !io::stdin().is_terminal() {
        return Err(msg("se necesita una terminal interactiva"));
    }
    println!();
    println!("  {}", bold(title));
    println!();
    for (i, item) in items.iter().enumerate() {
        println!("  {}) {}", i + 1, item);
    }
    println!();

    loop {
        print!("  elige [1-{}] (enter cancela): ", items.len());
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let answer = line.trim();
        if answer.is_empty() {
            return Ok(None);
        }
        if let Ok(n) = answer.parse::<usize>() {
            if (1..=items.len()).contains(&n) {
                return Ok(Some(n - 1));
            }
        }
        println!("  {}", dim("opción inválida"));
    }
}

pub fn progress(text: &str) {
    if io::stderr().is_terminal() {
        eprintln!("{}", dim(text));
    }
}

pub fn prompt(label: &str, default: &str) -> Result<String> {
    let suffix = if default.is_empty() {
        String::new()
    } else {
        format!(" {}", dim(&format!("[{default}]")))
    };
    print!("  {label}{suffix}: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let answer = line.trim();
    Ok(if answer.is_empty() {
        default.to_string()
    } else {
        answer.to_string()
    })
}
