use super::{Filter, Offer, Request};
use crate::{AppError, AppResult};
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Args)]
pub struct ChooserArgs {
    #[command(subcommand)]
    pub command: ChooserCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum ChooserCommand {
    Offer {
        #[arg(long)]
        document: String,
    },
    Watch {
        #[arg(long, default_value_t = 0)]
        revision: u64,
    },
    Choose {
        #[arg(long)]
        handle: String,
        #[arg(long)]
        path: Vec<PathBuf>,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        overwrite: bool,
    },
    Cancel {
        #[arg(long)]
        handle: String,
    },
    Filter {
        #[arg(long)]
        document: String,
    },
}

struct Pending {
    offer: Offer,
    request: Mutex<Request>,
    cancelled: AtomicBool,
}

#[derive(Default)]
struct State {
    pending: BTreeMap<String, Arc<Pending>>,
    revision: u64,
}

fn broker() -> &'static (Mutex<State>, Condvar) {
    static BROKER: OnceLock<(Mutex<State>, Condvar)> = OnceLock::new();
    BROKER.get_or_init(|| {
        (
            Mutex::new(State {
                revision: 1,
                ..State::default()
            }),
            Condvar::new(),
        )
    })
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn interrupt() {
    let _state = lock(&broker().0);
    broker().1.notify_all();
}

fn changed() {
    lock(&broker().0).revision += 1;
    broker().1.notify_all();
}

fn pending(handle: &str) -> AppResult<Arc<Pending>> {
    lock(&broker().0)
        .pending
        .get(handle)
        .cloned()
        .ok_or_else(|| AppError::invalid("Chooser request is no longer pending"))
}

pub fn run(args: ChooserArgs, cancelled: &AtomicBool) -> Value {
    match execute(args.command, cancelled) {
        Ok(value) => value,
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    }
}

fn execute(command: ChooserCommand, cancelled: &AtomicBool) -> AppResult<Value> {
    match command {
        ChooserCommand::Offer { document } => {
            let offer: Offer = serde_json::from_str(&document)?;
            let request = Request::new(offer.clone())?;
            let item = Arc::new(Pending {
                offer,
                request: Mutex::new(request),
                cancelled: AtomicBool::new(false),
            });
            {
                let mut state = lock(&broker().0);
                if state.pending.len() >= 16 || state.pending.contains_key(&item.offer.handle) {
                    return Err(AppError::invalid(
                        "Chooser request limit or duplicate handle",
                    ));
                }
                state
                    .pending
                    .insert(item.offer.handle.clone(), Arc::clone(&item));
                state.revision += 1;
                broker().1.notify_all();
            }
            let outcome = loop {
                if cancelled.load(Ordering::Relaxed) {
                    item.cancelled.store(true, Ordering::Relaxed);
                }
                let request = match item.request.try_lock() {
                    Ok(request) => Some(request),
                    Err(std::sync::TryLockError::Poisoned(error)) => Some(error.into_inner()),
                    Err(std::sync::TryLockError::WouldBlock) => None,
                };
                if let Some(mut request) = request {
                    if item.cancelled.load(Ordering::Relaxed) {
                        request.cancel();
                    }
                    if let Some(outcome) = request.outcome() {
                        break outcome.clone();
                    }
                }
                let state = lock(&broker().0);
                drop(
                    broker()
                        .1
                        .wait_timeout(state, Duration::from_millis(100))
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
            };
            lock(&broker().0).pending.remove(&item.offer.handle);
            changed();
            Ok(json!({"ok": true, "outcome": outcome}))
        }
        ChooserCommand::Watch { revision } => {
            let until = Instant::now() + Duration::from_secs(15);
            let mut state = lock(&broker().0);
            while state.revision == revision
                && !cancelled.load(Ordering::Relaxed)
                && Instant::now() < until
            {
                state = broker()
                    .1
                    .wait_timeout(state, until.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0;
            }
            if cancelled.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            Ok(
                json!({"ok": true, "revision": state.revision, "offers": state.pending.values().map(|item| &item.offer).collect::<Vec<_>>()}),
            )
        }
        ChooserCommand::Choose {
            handle,
            path,
            filter,
            overwrite,
        } => {
            let item = pending(&handle)?;
            let filter: Option<Filter> =
                filter.map(|text| serde_json::from_str(&text)).transpose()?;
            let result = lock(&item.request).choose_cancellable(
                &path,
                filter.as_ref(),
                overwrite,
                &item.cancelled,
            )?;
            broker().1.notify_all();
            Ok(json!({"ok": true, "decision": result}))
        }
        ChooserCommand::Cancel { handle } => {
            let item = pending(&handle)?;
            item.cancelled.store(true, Ordering::Relaxed);
            let accepted = lock(&item.request).cancel();
            broker().1.notify_all();
            Ok(json!({"ok": true, "cancelled": accepted}))
        }
        ChooserCommand::Filter { document } => {
            #[derive(Deserialize)]
            struct Rows {
                filter: Filter,
                entries: Vec<Row>,
            }
            #[derive(Deserialize)]
            struct Row {
                path: String,
                name: String,
                mime: String,
            }
            let rows: Rows = serde_json::from_str(&document)?;
            let allowed: Vec<_> = rows
                .entries
                .into_iter()
                .filter(|row| rows.filter.allows(&row.name, &row.mime))
                .map(|row| row.path)
                .collect();
            Ok(json!({"ok": true, "paths": allowed}))
        }
    }
}
