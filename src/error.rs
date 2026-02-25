use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    Config(#[from] toml::de::Error),
    #[error("jpeg encode error: {0}")]
    Jpeg(String),
    #[error("emulator error: {0}")]
    Emulator(String),
    #[error("save state error: {0}")]
    SaveState(String),
}
