use crate::config::{QueueConfig, RedisConfig};
use crate::connection_supervisor::ConnectionState;
use crate::error::JobResult;
use crate::metrics::MetricsRegistry;
use crate::pool::RedisPool;
use crate::queue::JobQueue;
use crate::retry::RetryConfig;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

type Handler = Arc<dyn Fn(Vec<u8>) -> JobResult<()> + Send + Sync>;

struct WorkerConfig {
    queue: String,
    handler: Handler,
    concurrency: usize,
    pool_size: Option<usize>,
    min_idle: Option<usize>,
}

struct QueueGroup {
    pool: Arc<RedisPool>,
    queue: Arc<JobQueue>,
    workers: Vec<(Handler, usize)>,
}

pub struct SubscriberRegistry {
    redis_config: RedisConfig,
    retry_config: RetryConfig,
    metrics: Arc<MetricsRegistry>,
    workers: Vec<WorkerConfig>,
}

impl SubscriberRegistry {
    pub fn new() -> Self {
        Self {
            redis_config: RedisConfig::new("redis://127.0.0.1:6379"),
            retry_config: RetryConfig::new()
                .with_max_attempts(20)
                .with_initial_delay(500)
                .with_max_delay(30000),
            metrics: Arc::new(MetricsRegistry::new()),
            workers: Vec::new(),
        }
    }

    pub fn with_redis(mut self, url: impl Into<String>) -> Self {
        self.redis_config = RedisConfig::new(url);
        self
    }

    pub fn register<F>(mut self, queue: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Vec<u8>) -> JobResult<()> + Send + Sync + 'static,
    {
        self.workers.push(WorkerConfig {
            queue: queue.into(),
            handler: Arc::new(handler),
            concurrency: 1,
            pool_size: None,
            min_idle: None,
        });
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        if let Some(worker) = self.workers.last_mut() {
            worker.concurrency = concurrency;
        }
        self
    }

    pub fn with_pool_size(mut self, pool_size: usize) -> Self {
        if let Some(worker) = self.workers.last_mut() {
            worker.pool_size = Some(pool_size);
        }
        self
    }

    pub fn with_min_idle(mut self, min_idle: usize) -> Self {
        if let Some(worker) = self.workers.last_mut() {
            worker.min_idle = Some(min_idle);
        }
        self
    }

    pub async fn run(self) -> JobResult<()> {
        println!("\n🔄 SubscriberRegistry starting...");
        println!("   📊 Metrics enabled");
        println!("   🔗 Connection supervisor (RabbitMQ-style)\n");
        println!("Press Ctrl+C to stop\n");

        let mut queue_groups: HashMap<String, QueueGroup> = HashMap::new();

        for worker_config in self.workers {
            if !queue_groups.contains_key(&worker_config.queue) {
                let pool = Arc::new(
                    RedisPool::with_config(
                        self.redis_config.clone(),
                        &worker_config.queue,
                        self.metrics.clone(),
                        worker_config.pool_size,
                        worker_config.min_idle,
                    )
                    .await?,
                );

                let queue = Arc::new(
                    JobQueue::new(
                        QueueConfig::new(&worker_config.queue),
                        self.redis_config.clone(),
                    )
                    .await?
                    .with_retry_config(self.retry_config.clone()),
                );

                println!(
                    "✓ Queue '{}' initialized with supervisor (max: {})\n",
                    worker_config.queue,
                    pool.status().await.max_size
                );

                queue_groups.insert(
                    worker_config.queue.clone(),
                    QueueGroup {
                        pool,
                        queue,
                        workers: Vec::new(),
                    },
                );
            }

            if let Some(group) = queue_groups.get_mut(&worker_config.queue) {
                group.workers.push((worker_config.handler, worker_config.concurrency));
            }
        }

        let mut handles = Vec::new();

        for (queue_name, group) in queue_groups {
            for (worker_idx, (handler, concurrency)) in group.workers.into_iter().enumerate() {
                for i in 0..concurrency {
                    let queue_clone = group.queue.clone();
                    let handler = handler.clone();
                    let pool = group.pool.clone();
                    let queue_name = queue_name.clone();
                    let metrics = self.metrics.clone();

                    let handle = tokio::spawn(async move {
                        println!(
                            "📝 Worker #{}-{} started for queue: {}",
                            worker_idx, i, queue_name
                        );

                        loop {
                            if let Ok(Some(job)) = queue_clone.dequeue::<Value>().await {
                                println!("\n🎉 JOB RECEIVED!");
                                println!("   Job ID: {}", job.id);
                                println!("   Queue: {}", job.queue);

                                let data = serde_json::to_vec(&job.payload).unwrap_or_default();

                                let start = std::time::Instant::now();
                                let result = handler(data);
                                let latency = start.elapsed().as_millis() as f64;

                                if let Err(e) = result {
                                    println!("   ❌ Error: {}", e);
                                    metrics
                                        .record_operation(&queue_name, latency, false)
                                        .await;
                                } else {
                                    metrics
                                        .record_operation(&queue_name, latency, true)
                                        .await;
                                }

                                // Log supervisor status
                                let conn_state = pool.supervisor().state().await;
                                if conn_state != ConnectionState::Connected {
                                    println!("   ⚠️  Connection state: {:?}", conn_state);
                                }
                            }

                            sleep(Duration::from_millis(500)).await;
                        }
                    });

                    handles.push(handle);
                }
            }
        }

        for handle in handles {
            handle.await?;
        }

        Ok(())
    }

    pub async fn get_pool_status(&self, queue_name: &str) -> Option<crate::metrics::PoolStatus> {
        self.metrics.get_pool_status(queue_name).await
    }

    pub async fn get_metrics(&self, queue_name: &str) -> Option<PerformanceMetrics> {
        self.metrics.get_metrics(queue_name).await
    }

    pub async fn get_all_metrics(&self) -> HashMap<String, PerformanceMetrics> {
        self.metrics.get_all_metrics().await
    }

    pub async fn health_check(&self, queue_name: &str) -> QueueHealth {
        self.metrics.health_check(queue_name).await
    }

    pub async fn health_check_all(&self) -> HashMap<String, QueueHealth> {
        self.metrics.health_check_all().await
    }
}

impl Default for SubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub use crate::metrics::{QueueHealth, PerformanceMetrics};
