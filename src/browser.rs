use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};

use crate::api::Api;
use crate::config::{self, Config};
use crate::error::{msg, AppError, Result};
use crate::ui;

const LOGIN_URL: &str = "https://iengage.coforge.com/ess2/login";

// Chrome for Testing estable, usado solo cuando no hay navegador instalado.
// Fuente: https://googlechromelabs.github.io/chrome-for-testing/
const CFT_VERSION: &str = "158.0.8127.36";
const CFT_URL: &str = "https://storage.googleapis.com/chrome-for-testing-public/158.0.8127.36/linux64/chrome-linux64.zip";
const CFT_SHA256: &str = "dba9cc838e1ce269c33ea68170881127bd15d078fc3ca5891c90802819a4d7c7";

pub fn login(config: &Config, browser: Option<&str>, timeout_min: u64) -> Result<()> {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(msg("no hay entorno gráfico · usa timesheet login --cookie"));
    }

    let path = resolve_browser(browser)?;
    ui::say("→ Abriendo iEngage en el navegador…");

    let mut session = Session::launch(&path)?;
    ui::say("  Inicia sesión con tu cuenta. La ventana se cerrará sola.");

    let deadline = Instant::now() + Duration::from_secs(timeout_min * 60);

    loop {
        if let Ok(header) = session.cookies() {
            if !header.is_empty() {
                let api = Api::new(config, header.clone());
                if api.verify().is_ok() {
                    config::save_cookie(&header)?;
                    ui::ok("sesión guardada");
                    return Ok(());
                }
            }
        }

        if session.exited() {
            return Err(msg("cerraste el navegador antes de completar la sesión"));
        }
        if Instant::now() >= deadline {
            return Err(msg(format!(
                "no se detectó la sesión en {timeout_min} min · vuelve a intentar"
            )));
        }
        sleep(Duration::from_secs(2));
    }
}

fn resolve_browser(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        let path = PathBuf::from(path);
        if !is_executable(&path) {
            return Err(msg(format!("no es un ejecutable: {}", path.display())));
        }
        return Ok(path);
    }
    if let Some(path) = find_system_browser() {
        return Ok(path);
    }
    if let Some(path) = cached_cft() {
        return Ok(path);
    }
    download_with_consent()
}

fn download_with_consent() -> Result<PathBuf> {
    if !io::stdin().is_terminal() {
        return Err(msg(
            "no hay navegador instalado · instala Chrome/Chromium o usa --cookie",
        ));
    }

    ui::warn("no encontré Chrome, Chromium ni Edge");
    let answer = ui::prompt(
        &format!("¿Descargar Chrome for Testing {CFT_VERSION} (~200 MB) solo para el login?"),
        "N",
    )?;
    if !is_yes(&answer) {
        return Err(msg("login cancelado · instala un navegador o usa --cookie"));
    }

    download_cft()?;
    cached_cft().ok_or_else(|| msg("la descarga terminó pero no encontré el navegador"))
}

fn find_system_browser() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TIMESHEET_BROWSER") {
        let path = PathBuf::from(path);
        if is_executable(&path) {
            return Some(path);
        }
    }

    let names = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ];
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for name in names {
                let candidate = dir.join(name);
                if is_executable(&candidate) && !is_sandboxed(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    [
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/opt/google/chrome/chrome",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| is_executable(path) && !is_sandboxed(path))
}

fn is_sandboxed(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains("/snap/") || path.contains("/flatpak/") || path.contains("/var/lib/flatpak/")
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
        && path
            .metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("TIMESHEET_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(config::home()?
        .join(".local")
        .join("share")
        .join("timesheet"))
}

fn cft_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("chrome").join(CFT_VERSION))
}

fn cached_cft() -> Option<PathBuf> {
    let path = cft_dir().ok()?.join("chrome-linux64").join("chrome");
    is_executable(&path).then_some(path)
}

fn download_cft() -> Result<()> {
    let dir = cft_dir()?;
    if cached_cft().is_some() {
        return Ok(());
    }
    fs::create_dir_all(&dir)?;

    let zip_path = dir.join("chrome-linux64.zip.part");
    ui::say(&format!(
        "→ descargando Chrome for Testing {CFT_VERSION} (~200 MB)…"
    ));
    download(CFT_URL, &zip_path)?;

    let actual = sha256_file(&zip_path)?;
    if actual != CFT_SHA256 {
        let _ = fs::remove_file(&zip_path);
        return Err(msg(format!(
            "checksum inválido del navegador (esperado {CFT_SHA256}, obtenido {actual})"
        )));
    }
    ui::say("✔ checksum verificado");
    ui::say("→ extrayendo…");

    let file = File::open(&zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| msg(format!("zip inválido: {err}")))?;
    archive
        .extract(&dir)
        .map_err(|err| msg(format!("no se pudo extraer el navegador: {err}")))?;
    let _ = fs::remove_file(&zip_path);

    let chrome = dir.join("chrome-linux64").join("chrome");
    if !chrome.is_file() {
        return Err(msg("la extracción no produjo el binario de Chrome"));
    }
    for name in [
        "chrome",
        "chrome_crashpad_handler",
        "chrome_sandbox",
        "chrome-wrapper",
    ] {
        let path = dir.join("chrome-linux64").join(name);
        if path.is_file() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
    }

    ui::ok("navegador listo");
    Ok(())
}

fn download(url: &str, path: &Path) -> Result<()> {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(900)))
        .tls_config(tls)
        .build()
        .new_agent();

    let response = agent
        .get(url)
        .call()
        .map_err(|err| AppError::Network(err.to_string()))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    let mut body = response.into_body().into_reader();
    let mut file = File::create(path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    let mut last_report = Instant::now();

    loop {
        let read = body
            .read(&mut buffer)
            .map_err(|err| AppError::Network(err.to_string()))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        written += read as u64;
        if last_report.elapsed() >= Duration::from_millis(200) {
            progress(written, total);
            last_report = Instant::now();
        }
    }
    progress(written, total);
    if io::stderr().is_terminal() {
        eprintln!();
    }
    Ok(())
}

fn progress(done: u64, total: u64) {
    if !io::stderr().is_terminal() {
        return;
    }
    let megabytes = done as f64 / 1_000_000.0;
    match done.saturating_mul(100).checked_div(total) {
        Some(percent) => eprint!(
            "\r  {percent:>3}% ({megabytes:.0} MB / {:.0} MB)",
            total as f64 / 1_000_000.0
        ),
        None => eprint!("\r  {megabytes:.0} MB"),
    }
    let _ = io::stderr().flush();
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_yes(answer: &str) -> bool {
    matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    )
}

fn login_url() -> String {
    std::env::var("TIMESHEET_LOGIN_URL").unwrap_or_else(|_| LOGIN_URL.to_string())
}

fn parse_active_port(text: &str) -> Option<(u16, String)> {
    let mut lines = text.lines();
    let port = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some((port, path))
    }
}

struct Session {
    child: Child,
    profile: PathBuf,
    port: u16,
    ws_path: String,
    log: PathBuf,
}

impl Session {
    fn launch(browser: &Path) -> Result<Self> {
        let profile = temp_profile()?;
        fs::create_dir_all(&profile)?;
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o700))?;
        let crash_dir = profile.join("crash");
        fs::create_dir_all(&crash_dir)?;

        let log_path = profile.join("chrome.log");
        let log = File::create(&log_path)?;
        let child = Command::new(browser)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--remote-debugging-port=0")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-sync")
            .arg("--disable-default-apps")
            .arg("--disable-breakpad")
            .arg("--disable-crash-reporter")
            .arg(format!("--crash-dumps-dir={}", crash_dir.display()))
            .arg("--disable-features=Translate,MediaRouter")
            .arg("--disable-search-engine-choice-screen")
            .arg("--password-store=basic")
            .arg("--start-maximized")
            .arg(login_url())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|err| {
                let _ = fs::remove_dir_all(&profile);
                msg(format!("no se pudo abrir el navegador: {err}"))
            })?;

        let mut session = Self {
            child,
            profile,
            port: 0,
            ws_path: String::new(),
            log: log_path,
        };
        session.wait_devtools()?;
        Ok(session)
    }

    fn wait_devtools(&mut self) -> Result<()> {
        let port_file = self.profile.join("DevToolsActivePort");
        let deadline = Instant::now() + Duration::from_secs(60);

        loop {
            if let Ok(text) = fs::read_to_string(&port_file) {
                if let Some((port, path)) = parse_active_port(&text) {
                    self.port = port;
                    self.ws_path = path;
                    return Ok(());
                }
            }
            if let Some(status) = self.child.try_wait()? {
                return Err(
                    self.launch_error(format!("el navegador se cerró al iniciar ({status})"))
                );
            }
            if Instant::now() >= deadline {
                return Err(
                    self.launch_error("el navegador no expuso DevTools a tiempo".to_string())
                );
            }
            sleep(Duration::from_millis(150));
        }
    }

    fn launch_error(&self, message: String) -> AppError {
        let log = fs::read_to_string(&self.log).unwrap_or_default();
        let detail: Vec<&str> = log
            .lines()
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(2)
            .collect();
        if detail.is_empty() {
            msg(message)
        } else {
            msg(format!(
                "{message} · {}",
                detail.into_iter().rev().collect::<Vec<_>>().join(" ")
            ))
        }
    }

    fn cookies(&self) -> Result<String> {
        let value = self.cdp("Storage.getCookies", json!({}))?;
        let cookies = value
            .get("result")
            .and_then(|result| result.get("cookies"))
            .and_then(|cookies| cookies.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(cookie_header(&cookies))
    }

    fn cdp(&self, method: &str, params: Value) -> Result<Value> {
        let url = format!("ws://127.0.0.1:{}{}", self.port, self.ws_path);
        let (mut socket, _) =
            connect(url).map_err(|err| msg(format!("no se pudo conectar al navegador: {err}")))?;
        if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
        }

        let request = json!({ "id": 1, "method": method, "params": params });
        socket
            .send(Message::text(request.to_string()))
            .map_err(|err| msg(format!("no se pudo consultar el navegador: {err}")))?;

        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    let value: Value = serde_json::from_str(text.as_str())
                        .map_err(|_| msg("respuesta CDP ilegible"))?;
                    if value.get("id").and_then(|id| id.as_i64()) != Some(1) {
                        continue;
                    }
                    if let Some(error) = value.get("error") {
                        return Err(msg(format!("el navegador rechazó la consulta: {error}")));
                    }
                    return Ok(value);
                }
                Ok(Message::Close(_)) => {
                    return Err(msg("el navegador cerró la conexión"));
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(Value::Null);
                }
                Err(err) => {
                    return Err(msg(format!("conexión con el navegador: {err}")));
                }
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
        for _ in 0..5 {
            if fs::remove_dir_all(&self.profile).is_ok() {
                break;
            }
            sleep(Duration::from_millis(200));
        }
    }
}

impl Session {
    fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    fn close(&mut self) {
        if self.port > 0 {
            let url = format!("ws://127.0.0.1:{}{}", self.port, self.ws_path);
            if let Ok((mut socket, _)) = connect(url) {
                let _ = socket.send(Message::text(r#"{"id":99,"method":"Browser.close"}"#));
                let _ = socket.flush();
            }
        }
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn temp_profile() -> Result<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or(config::home()?.join(".cache"));
    let dir = cache.join("timesheet").join("login");
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(dir)
}

fn relevant_domain(domain: &str) -> bool {
    let host = "iengage.coforge.com";
    let domain = domain.trim_start_matches('.');
    !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
}

fn cookie_header(cookies: &[Value]) -> String {
    cookies
        .iter()
        .filter_map(|cookie| {
            let name = cookie.get("name")?.as_str()?;
            let value = cookie.get("value")?.as_str()?;
            let domain = cookie.get("domain")?.as_str()?;
            if value.is_empty() || !relevant_domain(domain) {
                return None;
            }
            Some(format!("{name}={value}"))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devtools_active_port() {
        let text = "39415\n/devtools/browser/abc-123\n";
        assert_eq!(
            parse_active_port(text),
            Some((39415, "/devtools/browser/abc-123".to_string()))
        );
        assert_eq!(parse_active_port("nope"), None);
        assert_eq!(parse_active_port("123\n"), None);
    }

    #[test]
    fn filters_cookie_domains() {
        assert!(relevant_domain("iengage.coforge.com"));
        assert!(relevant_domain(".iengage.coforge.com"));
        assert!(relevant_domain(".coforge.com"));
        assert!(!relevant_domain(".google.com"));
        assert!(!relevant_domain("iengageauth.coforge.com"));
    }

    #[test]
    fn builds_cookie_header() {
        let cookies = vec![
            json!({"name": "a", "value": "1", "domain": ".iengage.coforge.com"}),
            json!({"name": "b", "value": "2", "domain": ".coforge.com"}),
            json!({"name": "c", "value": "3", "domain": ".google.com"}),
            json!({"name": "d", "value": "", "domain": ".iengage.coforge.com"}),
        ];
        assert_eq!(cookie_header(&cookies), "a=1; b=2");
    }

    #[test]
    fn hashes_files() {
        let path = std::env::temp_dir().join(format!("timesheet-hash-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn accepts_spanish_yes() {
        assert!(is_yes("s"));
        assert!(is_yes("Sí"));
        assert!(is_yes("yes"));
        assert!(!is_yes("n"));
    }
}
