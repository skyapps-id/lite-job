# RabbitMQ-Style Connection Management

## Overview

Lite-job now uses **centralized connection management** similar to RabbitMQ, eliminating retry storms and providing clean logs.

## Before vs After

### ❌ Before (Per-Operation Retry)
```
10 Workers → Each calls dequeue()
    ↓
Each → retry_async() independently
    ↓
Redis dies → 10 concurrent retry loops
    ↓
Logs:
  ⚠️ Redis operation failed (attempt 1/20) × 10 workers
  ⚠️ Redis operation failed (attempt 2/20) × 10 workers
  ⚠️ Redis operation failed (attempt 3/20) × 10 workers
  ... (200+ log messages!)
```

### ✅ After (Connection Supervisor)
```
Connection Supervisor (background task)
    ↓
Maintains connection state
    ↓
Redis dies → 1 retry loop (logged once)
    ↓
Workers wait for "ready" signal
    ↓
Logs:
  ⚠️ Redis disconnected - starting reconnection (1 log)
  ⚠️ Reconnect attempt 1/20 failed (1 log)
  ⚠️ Reconnect attempt 2/20 failed (1 log)
  ✅ Redis reconnected successfully (1 log)
  ... (5 log messages total!)
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  ConnectionSupervisor (per pool)            │
│  ┌────────────────────────────────────────┐ │
│  │ State: Connected/Connecting/Disconnected │
│  │ Background supervision loop             │
│  │ Single retry logic                      │
│  └────────────────────────────────────────┘ │
│                    ↕                         │
│         wait_ready() / get_connection()      │
└─────────────────────────────────────────────┘
                          ↕
              ┌─────────────────┐
              │   RedisPool      │
              │   (wraps         │
              │    supervisor)   │
              └─────────────────┘
                    ↕
         ┌──────────────────────┐
         │ SubscriberRegistry    │
         │                      │
         │ ┌────┐ ┌────┐ ┌────┐│
         │ │W1  │ │W2  │ │W3  ││
         │ └────┘ └────┘ └────┘│
         └──────────────────────┘
```

## Features

1. **Single Retry Point**: Only supervisor retries connection
2. **Clean Logs**: 1 set of retry messages instead of N×workers
3. **No Thundering Herd**: Workers wait, don't compete
4. **Connection State Tracking**: Connected/Connecting/Disconnected
5. **Graceful Backoff**: Exponential backoff on repeated failures
6. **Automatic Recovery**: Reconnects and notifies all workers

## Usage

```rust
let registry = SubscriberRegistry::new()
    .register("orders", handle_orders)
        .with_pool_size(20)
        .with_concurrency(5);

registry.run().await?;
```

**Behind the scenes:**
1. Creates `ConnectionSupervisor` per queue
2. Supervisor starts background task
3. Workers call `supervisor.wait_ready()` before operations
4. Single retry loop maintains connection
5. Clean logs!

## Testing

Run: `cargo run --example queue_consumer`

**Stop Redis:**
```
⚠️ Redis disconnected - starting reconnection  (1 log, not 10!)
⚠️ Reconnect attempt 1/20 failed: Connection refused
⚠️ Reconnect attempt 2/20 failed: Connection refused
✅ Redis connected - notifying workers
```

**Start Redis:**
```
✅ Redis reconnected successfully
📝 Worker #0-0 continues processing
📝 Worker #0-1 continues processing
...
```

## Benefits

| Metric | Before | After |
|--------|--------|-------|
| Retry logs when Redis down (10 workers) | 200+ | 5-10 |
| Concurrent retry attempts | 10 | 1 |
| Connection management | Distributed | Centralized |
| Log clarity | Spam | Clean |
| RabbitMQ-style | ❌ | ✅ |
