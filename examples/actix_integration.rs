use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{Duration, Utc};
use liteq::{JobQueue, QueueConfig, RedisConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: u32,
    text: String,
}

async fn enqueue_job() -> impl Responder {
    let config = RedisConfig::new("redis://127.0.0.1:6379");
    let queue_config = QueueConfig::new("orders");
    let queue = match JobQueue::new(queue_config, config).await {
        Ok(q) => q,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(format!("Failed to create queue: {}", e));
        }
    };

    let task = Task {
        id: 1,
        text: "Process order from actix-web".to_string(),
    };

    match queue.enqueue(task).send().await {
        Ok(job_id) => HttpResponse::Ok().json(format!("Job enqueued: {}", job_id)),
        Err(e) => HttpResponse::InternalServerError().json(format!("Failed to enqueue: {}", e)),
    }
}

async fn enqueue_scheduled_job() -> impl Responder {
    let config = RedisConfig::new("redis://127.0.0.1:6379");
    let queue_config = QueueConfig::new("orders");
    let queue = match JobQueue::new(queue_config, config).await {
        Ok(q) => q,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(format!("Failed to create queue: {}", e));
        }
    };

    let task = Task {
        id: 2,
        text: "Send scheduled email".to_string(),
    };

    let eta = Utc::now() + Duration::seconds(10);

    match queue.enqueue(task).with_eta(eta).send().await {
        Ok(job_id) => HttpResponse::Ok().json(format!("Scheduled job enqueued: {}", job_id)),
        Err(e) => HttpResponse::InternalServerError().json(format!("Failed to enqueue: {}", e)),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    println!("🚀 Actix-web + liteq Integration Demo");
    println!("Server running at http://127.0.0.1:8080");
    println!();
    println!("Endpoints:");
    println!("  GET /enqueue         - Enqueue immediate job");
    println!("  GET /scheduled      - Enqueue scheduled job (+10s)");
    println!();

    HttpServer::new(|| {
        App::new()
            .route("/enqueue", web::get().to(enqueue_job))
            .route("/scheduled", web::get().to(enqueue_scheduled_job))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
