use lite_job_redis::{JobResult, SubscriberRegistry};

fn handle_orders(data: Vec<u8>) -> JobResult<()> {
    let msg = String::from_utf8_lossy(&data);
    println!("📦 Order received: {}", msg);
    Ok(())
}

fn handle_logs(data: Vec<u8>) -> JobResult<()> {
    let msg = String::from_utf8_lossy(&data);
    println!("📊 Log: {}", msg);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🚀 RabbitMQ-Style Consumer (Centralized Connection Management)\n");
    println!("Features:");
    println!("  ✓ Connection supervisor per pool");
    println!("  ✓ Single retry loop (logged once)");
    println!("  ✓ Workers wait for 'ready' signal");
    println!("  ✓ Clean logs - no retry spam!\n");

    let registry = SubscriberRegistry::new()
        // Order queue with larger pool (high traffic)
        .register("orders", handle_orders)
            .with_pool_size(20)
            .with_concurrency(5)
        // Log queue with smaller pool (low traffic)
        .register("logs", handle_logs)
            .with_pool_size(5)
            .with_concurrency(2);

    println!("💡 Stop Redis to see:");
    println!("  1. Only 1 '⚠️ Redis disconnected' log (not 10x)");
    println!("  2. Only 1 retry loop in background");
    println!("  3. Workers wait, don't spam retries");
    println!("  4. Single '✅ Redis connected' on recovery\n");

    registry.run().await?;
    Ok(())
}

