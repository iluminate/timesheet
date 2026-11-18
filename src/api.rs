use std::thread::sleep;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use serde_json::{json, Value};
use ureq::tls::{RootCerts, TlsConfig};

use crate::config::Config;
use crate::dates::{from_api, today, ymd};
use crate::error::{msg, AppError, Result};
use crate::log;

pub const ORIGIN: &str = "https://iengage.coforge.com";
const UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const RETRY_SECONDS: [u64; 3] = [5, 15, 30];

#[derive(Clone, Debug)]
pub struct Period {
    pub start_api: String,
    pub end_api: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

fn agent() -> ureq::Agent {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .max_redirects(0)
        .user_agent(UA)
        .tls_config(tls)
        .build()
        .new_agent()
}

/// Cliente de iEngage. La cookie viaja como header, igual que en la web.
#[derive(Clone)]
pub struct Api {
    agent: ureq::Agent,
    base: String,
    cookie: String,
    emp: String,
}

impl Api {
    pub fn new(config: &Config, cookie: String) -> Self {
        Self {
            agent: agent(),
            base: config.base_url.trim_end_matches('/').to_string(),
            cookie,
            emp: config.emp_code.clone(),
        }
    }

    fn request(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/{}", self.base, path);
        let payload = serde_json::to_string(body)?;
        let mut response = self
            .agent
            .post(&url)
            .header("accept", "application/json, text/javascript, */*; q=0.01")
            .header("accept-language", "es-419,es;q=0.9")
            .header("content-type", "application/json; charset=UTF-8")
            .header("cookie", &self.cookie)
            .header("origin", ORIGIN)
            .header("referer", format!("{ORIGIN}/"))
            .header("x-requested-with", "XMLHttpRequest")
            .send(payload.as_str())
            .map_err(|err| AppError::Network(err.to_string()))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if (300..400).contains(&status) {
            return Err(AppError::Session(format!(
                "redirección {status}{}",
                if location.is_empty() {
                    String::new()
                } else {
                    format!(" -> {location}")
                }
            )));
        }
        if !content_type.contains("application/json") {
            return Err(AppError::Session(format!(
                "respuesta no JSON (HTTP {status}, {})",
                if content_type.is_empty() {
                    "sin content-type"
                } else {
                    &content_type
                }
            )));
        }
        if status >= 400 {
            return Err(msg(format!("{path}: HTTP {status}")));
        }

        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|err| AppError::Network(err.to_string()))?;
        let json: Value = serde_json::from_str(&text)
            .map_err(|_| AppError::Session(format!("{path}: respuesta ilegible")))?;

        // iEngage responde HTTP 200 con un sobre de error cuando la sesión caduca.
        let code = json
            .get("StatusCode")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or(0);
        if code >= 400 {
            let detail = ["Exception", "ErrorMsg"]
                .iter()
                .find_map(|key| json.get(*key).and_then(|v| v.as_str()))
                .unwrap_or("")
                .trim()
                .to_string();
            return Err(AppError::Session(detail));
        }
        if json.get("Result").is_none() {
            let detail = json
                .get("Exception")
                .and_then(|v| v.as_str())
                .unwrap_or("respuesta sin Result")
                .trim()
                .to_string();
            return Err(AppError::Session(detail));
        }

        Ok(json)
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let mut attempt = 0;
        loop {
            match self.request(path, body) {
                Err(AppError::Network(detail)) if attempt < RETRY_SECONDS.len() => {
                    let wait = RETRY_SECONDS[attempt];
                    log::notice(&format!(
                        "sin red ({detail}); reintento {}/{} en {wait}s",
                        attempt + 1,
                        RETRY_SECONDS.len()
                    ));
                    sleep(Duration::from_secs(wait));
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    pub fn periods(&self, year: i32) -> Result<Vec<Period>> {
        let out = self.post(
            "BindControls",
            &json!({ "_InputData": { "Type": 1, "Year": year.to_string() } }),
        )?;
        let Some(rows) = out.get("Result").and_then(|v| v.as_array()) else {
            return Ok(Vec::new());
        };
        Ok(rows
            .iter()
            .filter(|row| {
                row.get("SearchText")
                    .and_then(|v| v.as_str())
                    .map(|v| v.trim().eq_ignore_ascii_case("WEEKS"))
                    .unwrap_or(false)
            })
            .filter_map(parse_period)
            .collect())
    }

    pub fn current_period(&self, date: NaiveDate) -> Result<Period> {
        let year = date.year();
        for candidate in [year, year - 1, year + 1] {
            for period in self.periods(candidate)? {
                if date >= period.start && date <= period.end {
                    return Ok(period);
                }
            }
        }
        Err(msg(format!("no hay periodo para {}", ymd(date))))
    }

    pub fn employee(&self, period: &Period) -> Result<Value> {
        let out = self.post(
            "GetEmpDetails",
            &json!({
                "_InputData": {
                    "Type": 2,
                    "StartDate": period.start_api,
                    "EndDate": period.end_api,
                    "EmpCode": self.emp
                }
            }),
        )?;
        out.get("Result")
            .and_then(|v| v.as_array())
            .and_then(|rows| rows.first())
            .cloned()
            .ok_or_else(|| msg("GetEmpDetails no devolvió datos del empleado."))
    }

    pub fn days(&self, period: &Period) -> Result<Vec<Value>> {
        let out = self.post(
            "GetMyTimesheetList",
            &json!({
                "_InputData": {
                    "Type": 3,
                    "StartDate": period.start_api,
                    "EndDate": period.end_api
                }
            }),
        )?;
        Ok(list_of(&out))
    }

    pub fn grid(&self, period: &Period) -> Result<Vec<Value>> {
        let out = self.post(
            "GetEmployeeTimesheetData",
            &json!({
                "_InputData": {
                    "Type": 4,
                    "StartDate": period.start_api,
                    "EndDate": period.end_api
                }
            }),
        )?;
        Ok(list_of(&out))
    }

    pub fn persist(&self, entries: &[Value], action: &str) -> Result<()> {
        let out = self.post(
            "SubmitTimesheetDetails",
            &json!({
                "_submitData": entries,
                "_actionType": action,
                "_actionTakenBy": "User",
                "_actionTakenFrom": "Web"
            }),
        )?;
        let output = out
            .get("Result")
            .and_then(|v| v.as_array())
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("OUTPUT"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_uppercase();
        if output != "SUCCESS" {
            return Err(msg(format!(
                "{action} no confirmado por el servidor (OUTPUT={})",
                if output.is_empty() {
                    "vacío".to_string()
                } else {
                    output
                }
            )));
        }
        Ok(())
    }

    pub fn verify(&self) -> Result<()> {
        if self.periods(today().year())?.is_empty() {
            return Err(AppError::Session(String::new()));
        }
        Ok(())
    }
}

fn list_of(out: &Value) -> Vec<Value> {
    out.get("Result")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Primer header Cookie de un archivo o pegado desde el navegador.
pub fn parse_cookie(raw: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))?;
    Some(
        line.strip_prefix("Cookie:")
            .or_else(|| line.strip_prefix("cookie:"))
            .unwrap_or(line)
            .trim()
            .to_string(),
    )
}

fn parse_period(row: &Value) -> Option<Period> {
    let raw = row.get("Value")?.as_str()?;
    let (start, end) = raw.split_once(" - ")?;
    let start = start.trim();
    let end = end.trim();
    Some(Period {
        start_api: start.to_string(),
        end_api: end.to_string(),
        start: from_api(start).ok()?,
        end: from_api(end).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    use super::*;

    fn serve_once(body: &str) -> (String, mpsc::Receiver<String>, JoinHandle<()>) {
        serve(format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        ))
    }

    fn serve(response: String) -> (String, mpsc::Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();

            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                let lower = line.to_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                request.push_str(&line);
            }
            let mut payload = vec![0; length];
            reader.read_exact(&mut payload).unwrap();
            request.push_str(&String::from_utf8_lossy(&payload));

            sender.send(request).unwrap();
            stream
                .try_clone()
                .unwrap()
                .write_all(response.as_bytes())
                .unwrap();
        });

        (format!("http://{address}"), receiver, handle)
    }

    fn api(base_url: String) -> Api {
        Api::new(
            &Config {
                base_url,
                emp_code: "123".into(),
                ..Config::default()
            },
            "a=b".into(),
        )
    }

    #[test]
    fn parses_periods_and_sends_cookie() {
        let body = r#"{"Result":[{"SearchText":"WEEKS","Value":"02-Nov-2026 - 08-Nov-2026"}]}"#;
        let (base_url, receiver, handle) = serve_once(body);

        let periods = api(base_url).periods(2026).unwrap();
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].start_api, "02-Nov-2026");
        assert_eq!(periods[0].end_api, "08-Nov-2026");

        let request = receiver.recv().unwrap();
        assert!(request.to_lowercase().contains("cookie: a=b"));
        assert!(request.contains(r#""Type":1"#), "request={request}");
        handle.join().unwrap();
    }

    #[test]
    fn detects_expired_session_envelope() {
        let body = r#"{"StatusCode":500,"Exception":"Timeout"}"#;
        let (base_url, _receiver, handle) = serve_once(body);

        let error = api(base_url).periods(2026).unwrap_err();
        assert!(matches!(error, AppError::Session(_)));
        handle.join().unwrap();
    }

    #[test]
    fn treats_redirect_as_expired_session() {
        let response = "HTTP/1.1 302 Found\r\nlocation: /ess2/login\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string();
        let (base_url, _receiver, handle) = serve(response);

        let error = api(base_url).periods(2026).unwrap_err();
        assert!(matches!(error, AppError::Session(_)));
        handle.join().unwrap();
    }

    #[test]
    fn rejects_failed_persist() {
        let body = r#"{"Result":[{"OUTPUT":"FAILED"}]}"#;
        let (base_url, _receiver, handle) = serve_once(body);

        let error = api(base_url).persist(&[], "Save").unwrap_err();
        assert!(error.message().contains("OUTPUT=FAILED"));
        handle.join().unwrap();
    }
}
