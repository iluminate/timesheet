use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Auth(String),
    Session(String),
    Network(String),
    Fatal(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

pub fn msg(message: impl Into<String>) -> AppError {
    AppError::Fatal(message.into())
}

impl AppError {
    pub fn message(&self) -> String {
        match self {
            AppError::Auth(detail) => match detail.is_empty() {
                true => "falta iniciar sesión · timesheet login".into(),
                false => format!("falta iniciar sesión · timesheet login · {detail}"),
            },
            AppError::Session(detail) => match detail.is_empty() {
                true => "sesión expirada · timesheet login".into(),
                false => format!("sesión expirada · timesheet login · {detail}"),
            },
            AppError::Network(detail) => format!("sin conexión · {detail}"),
            AppError::Fatal(message) => message.clone(),
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            AppError::Auth(_) | AppError::Session(_) => 2,
            AppError::Network(_) => 3,
            AppError::Fatal(_) => 1,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Fatal(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        AppError::Fatal(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Fatal(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Fatal(format!("respuesta ilegible: {value}"))
    }
}
