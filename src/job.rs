use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job<T> {
    /// Unique job identifier
    pub id: String,
    /// Job payload data
    pub payload: T,
    /// Target queue name
    pub queue: String,
    /// Job creation timestamp
    pub created_at: DateTime<Utc>,
    /// Optional scheduled execution time (ETA)
    pub eta: Option<DateTime<Utc>>,
    /// Current job status
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    /// Job is ready for immediate processing
    Pending,
    /// Job is scheduled for future execution
    Scheduled,
}

impl<T> Job<T> {
    /// Creates new job with payload and queue name
    pub fn new(payload: T, queue: impl Into<String>) -> Self
    where
        T: Serialize,
    {
        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            queue: queue.into(),
            created_at: Utc::now(),
            eta: None,
            status: JobStatus::Pending,
        }
    }

    /// Sets scheduled execution time (ETA)
    pub fn with_eta(mut self, eta: DateTime<Utc>) -> Self {
        self.eta = Some(eta);
        self.status = JobStatus::Scheduled;
        self
    }

    /// Serializes job to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error>
    where
        T: Serialize,
    {
        serde_json::to_string(self)
    }

    /// Deserializes job from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error>
    where
        T: DeserializeOwned,
    {
        serde_json::from_str(json)
    }
}
