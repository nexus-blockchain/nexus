use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    // 链交互
    #[error("Chain connection failed: {0}")]
    ChainConnection(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),

    // TEE
    #[error("Enclave error: {0}")]
    EnclaveError(String),
    #[error("Attestation failed: {0}")]
    AttestationFailed(String),
    #[error("Ceremony error: {0}")]
    CeremonyError(String),

    // 平台
    #[error("Platform API error: {platform} - {message}")]
    PlatformApi { platform: String, message: String },
    #[error("Webhook validation failed: {0}")]
    WebhookValidation(String),

    // 通用
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Channel send error: {0}")]
    ChannelSend(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type BotResult<T> = Result<T, BotError>;
