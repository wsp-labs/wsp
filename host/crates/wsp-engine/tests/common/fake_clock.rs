// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.test.ts:156 `fakeClock`: a clock whose sleeps cost nothing and move its own time, from 1,000,000.
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use wsp_engine::errors::RetryClock;

pub struct FakeClock {
    pub at: AtomicI64,
    /// Every sleep asked, in order.
    pub slept: Mutex<Vec<u64>>,
    /// What `random()` answers.
    pub draw: Mutex<f64>,
}

impl FakeClock {
    pub fn new() -> Arc<Self> {
        Self::at(1_000_000)
    }
    pub fn at(at: i64) -> Arc<Self> {
        Arc::new(FakeClock { at: AtomicI64::new(at), slept: Mutex::new(Vec::new()), draw: Mutex::new(0.0) })
    }
    pub fn now_ms(&self) -> i64 {
        self.at.load(Ordering::SeqCst)
    }
    pub fn advance(&self, ms: i64) {
        self.at.fetch_add(ms, Ordering::SeqCst);
    }
    pub fn set(&self, ms: i64) {
        self.at.store(ms, Ordering::SeqCst);
    }
    pub fn sleeps(&self) -> Vec<u64> {
        self.slept.lock().unwrap().clone()
    }
}

#[async_trait]
impl RetryClock for FakeClock {
    fn now(&self) -> i64 {
        self.at.load(Ordering::SeqCst)
    }
    async fn sleep(&self, ms: u64) {
        self.slept.lock().unwrap().push(ms);
        self.at.fetch_add(ms as i64, Ordering::SeqCst);
        tokio::task::yield_now().await;
    }
    fn random(&self) -> f64 {
        *self.draw.lock().unwrap()
    }
}
