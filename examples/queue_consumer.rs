use liteq::{JobResult, SubscriberRegistry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: u32,
    text: String,
}

#[derive(Clone)]
struct AppData {
    processed_count: Arc<std::sync::atomic::AtomicU64>,
}

impl AppData {
    fn new() -> Self {
        Self {
            processed_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn increment_counter(&self) {
        self.processed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn get_counter(&self) -> u64 {
        self.processed_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

async fn handle_orders(data: Vec<u8>, app_data: Arc<AppData>) -> JobResult<()> {
    let task: Task = serde_json::from_slice(&data)?;
    
    app_data.increment_counter();
    let count = app_data.get_counter();
    
    println!("📦 Order #{}: {} (Total: {})", task.id, task.text, count);
    
    Ok(())
}

async fn handle_logs(data: Vec<u8>, app_data: Arc<AppData>) -> JobResult<()> {
    let msg = String::from_utf8_lossy(&data);
    
    app_data.increment_counter();
    let count = app_data.get_counter();
    
    println!("📊 Log: {} (Total: {})", msg, count);
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🚀 Consumer Demo - Async Handlers with Dependency Injection\n");

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    
    let mut registry = SubscriberRegistry::new()
        .with_redis(redis_url);
    
    let app_data = Arc::new(AppData::new());
    let log_data = Arc::new(AppData::new());

    registry.register("orders", move |data| {
        let app_data = app_data.clone();
        async move {
            handle_orders(data, app_data).await
        }
    })
    .with_pool_size(20)
    .with_concurrency(1)
    .build();

    registry.register("logs", move |data| {
        let log_data = log_data.clone();
        async move {
            handle_logs(data, log_data).await
        }
    })
    .with_pool_size(5)
    .with_concurrency(2)
    .build();

    println!("⏳ Waiting for jobs...\n");

    registry.run().await?;
    
    Ok(())
}

