pub mod canonical;
pub mod emit;
pub mod frontmatter;
pub mod glob;
pub mod hooks;
pub mod mcp;
pub mod memory;
pub mod metrics;
pub mod path;
pub mod recovery_store;
pub mod skills;
pub mod snapshot;
pub mod text;
pub mod usage;
pub mod watch;

use crate::module_helpers::{CoreRoute, Request};
use crate::{AppError, AppResult};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub type CoreHandler = fn(&Request<'_>, &[String], &Context<'_>) -> AppResult<Value>;

pub struct Context<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl<'a> Context<'a> {
    pub fn new(timeout: Duration, cancelled: &'a AtomicBool) -> Self {
        Self {
            deadline: Instant::now() + timeout,
            cancelled,
        }
    }

    pub fn cancelled(&self) -> &'a AtomicBool {
        self.cancelled
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn check(&self) -> AppResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        if self.remaining().is_zero() {
            return Err(AppError::command("helper exceeded its time budget"));
        }
        Ok(())
    }
}

pub fn registered(route: CoreRoute, method: &str) -> Option<CoreHandler> {
    match route {
        CoreRoute::Skills => skills::handler(method),
        CoreRoute::Memory => memory::handler(method),
        CoreRoute::Hooks => hooks::handler(method),
        CoreRoute::Mcp => mcp::handler(method),
    }
}

pub fn dispatch(
    route: CoreRoute,
    request: &Request<'_>,
    arguments: &[String],
    context: &Context<'_>,
) -> Option<AppResult<Value>> {
    let handler = registered(route, request.method)?;
    Some(handler(request, arguments, context).and_then(|document| {
        if document.is_object() {
            Ok(document)
        } else {
            Err(AppError::command("helper response must be a JSON object"))
        }
    }))
}
