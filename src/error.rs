use thiserror::Error;

pub type JobResult<T> = Result<T, JobError>;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Redis connection error: {0}")]
    RedisConnection(#[from] redis::RedisError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Queue error: {0}")]
    QueueError(String),

    #[error("Worker error: {0}")]
    WorkerError(String),

    #[error("Handler not found for job: {0}")]
    HandlerNotFound(String),

    #[error("Job processing failed: {0}")]
    ProcessingFailed(String),

    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Task join error: {0}")]
    TaskJoinError(#[from] tokio::task::JoinError),

    #[error("Pool exhausted: {0}")]
    PoolExhausted(String),

    #[error("Connection timeout after {0}s")]
    ConnectionTimeout(u64),

    #[error("Pool closed")]
    PoolClosed,

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Pool error: {0}")]
    PoolError(#[from] deadpool::managed::PoolError<redis::RedisError>),
}
