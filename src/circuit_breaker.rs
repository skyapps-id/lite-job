use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU32>,
    success_count: Arc<AtomicU32>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold: 2,
            timeout,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU32::new(0)),
            success_count: Arc::new(AtomicU32::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    pub fn try_is_request_allowed(&self) -> bool {
        match self.state.try_read() {
            Ok(state) => {
                match *state {
                    CircuitState::Closed => true,
                    CircuitState::Open => {
                        match self.last_failure_time.try_read() {
                            Ok(last_failure) => {
                                if let Some(failure_time) = *last_failure {
                                    if failure_time.elapsed() >= self.timeout {
                                        drop(state);
                                        if let Ok(mut state_w) = self.state.try_write() {
                                            *state_w = CircuitState::HalfOpen;
                                            self.success_count.store(0, Ordering::Relaxed);
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            }
                            Err(_) => true,
                        }
                    }
                    CircuitState::HalfOpen => true,
                }
            }
            Err(_) => true,
        }
    }

    pub fn try_record_success(&self) {
        if let Ok(state) = self.state.try_read() {
            if *state == CircuitState::HalfOpen {
                let current = self.success_count.fetch_add(1, Ordering::Relaxed);
                
                if current + 1 >= self.success_threshold {
                    drop(state);
                    if let Ok(mut state_w) = self.state.try_write() {
                        if *state_w == CircuitState::HalfOpen {
                            *state_w = CircuitState::Closed;
                            self.failure_count.store(0, Ordering::Relaxed);
                            self.success_count.store(0, Ordering::Relaxed);
                        }
                    }
                }
            } else {
                self.failure_count.store(0, Ordering::Relaxed);
            }
        }
    }

    pub fn try_state(&self) -> CircuitState {
        self.state.try_read()
            .map(|state| state.clone())
            .unwrap_or(CircuitState::Closed)
    }

    pub fn try_record_failure(&self) {
        let current = self.failure_count.fetch_add(1, Ordering::Relaxed);

        if current + 1 >= self.failure_threshold {
            if let Ok(mut state) = self.state.try_write() {
                if *state != CircuitState::Open {
                    *state = CircuitState::Open;
                    if let Ok(mut last_failure) = self.last_failure_time.try_write() {
                        *last_failure = Some(Instant::now());
                        tracing::warn!("Circuit breaker OPEN after {} failures", current + 1);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let breaker = CircuitBreaker::new(3, Duration::from_secs(5));
        
        assert!(breaker.try_is_request_allowed());
        
        breaker.try_record_failure();
        breaker.try_record_failure();
        assert!(breaker.try_is_request_allowed());
        
        breaker.try_record_failure();
        assert!(!breaker.try_is_request_allowed());
        assert_eq!(breaker.try_state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_transitions_to_half_open() {
        let breaker = CircuitBreaker::new(2, Duration::from_millis(100));
        
        breaker.try_record_failure();
        breaker.try_record_failure();
        
        assert_eq!(breaker.try_state(), CircuitState::Open);
        assert!(!breaker.try_is_request_allowed());
        
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        assert!(breaker.try_is_request_allowed());
        assert_eq!(breaker.try_state(), CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_after_successes() {
        let breaker = CircuitBreaker::new(2, Duration::from_millis(100));
        
        breaker.try_record_failure();
        breaker.try_record_failure();
        
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        breaker.try_is_request_allowed();
        breaker.try_record_success();
        breaker.try_record_success();
        
        assert_eq!(breaker.try_state(), CircuitState::Closed);
    }
}
