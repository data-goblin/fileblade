use super::*;

pub(super) fn release_freed_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        libc::malloc_trim(0);
    }
}

pub(super) fn monitor_deadlines(active: Arc<Active>, stopping: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut idle_since = None;
        let mut last_sweep: Option<Instant> = None;
        let mut armed = false;
        while !stopping.load(Ordering::Relaxed) {
            active.changed.clear();
            let now = Instant::now();
            let mut busy = false;
            let mut next_deadline: Option<Instant> = None;
            for request in active.lock().values() {
                busy |= !request.standing;
                let Some(deadline) = request.deadline.as_ref() else {
                    continue;
                };
                if deadline.expired(now) {
                    request.deadline_exceeded.store(true, Ordering::Relaxed);
                    if request.cancel_on_deadline {
                        request.cancel();
                    }
                } else if !request.deadline_exceeded.load(Ordering::Relaxed) {
                    let at = deadline.next();
                    next_deadline = Some(next_deadline.map_or(at, |current| current.min(at)));
                }
            }
            let mut next_sweep = None;
            if busy {
                idle_since = None;
                armed = true;
            } else {
                let since = *idle_since.get_or_insert(now);
                let due = last_sweep
                    .is_none_or(|previous| now.duration_since(previous) >= IDLE_SWEEP_INTERVAL);
                if now.duration_since(since) >= IDLE_TRIM_DELAY && (armed || due) {
                    crate::index::sweep_idle();
                    release_freed_memory();
                    armed = false;
                    last_sweep = Some(now);
                }
                let settled = since + IDLE_TRIM_DELAY;
                next_sweep = if armed {
                    Some(settled)
                } else if crate::index::has_slots() {
                    Some(settled.max(last_sweep.map_or(settled, |at| at + IDLE_SWEEP_INTERVAL)))
                } else {
                    None
                };
            }
            let timeout = [next_deadline, next_sweep]
                .into_iter()
                .flatten()
                .min()
                .map(|at| {
                    at.saturating_duration_since(Instant::now())
                        .max(MONITOR_MIN_WAIT)
                });
            let mut descriptors = [PollFd::from_borrowed_fd(active.changed.fd(), PollFlags::IN)];
            if wake::wait(&mut descriptors, timeout).is_err() {
                thread::sleep(MONITOR_MIN_WAIT);
            }
        }
    })
}

pub(super) fn shutdown(
    active: &Arc<Active>,
    stopping: &Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    monitor: JoinHandle<()>,
) {
    for request in active.lock().values() {
        request.cancel();
    }
    for worker in workers {
        let _ = worker.join();
    }
    crate::hyprland::restore_owned_borders();
    stopping.store(true, Ordering::Relaxed);
    active.changed();
    let _ = monitor.join();
}

pub(super) fn reap(workers: &mut Vec<JoinHandle<()>>) {
    let mut pending = Vec::with_capacity(workers.len());
    for worker in workers.drain(..) {
        if worker.is_finished() {
            let _ = worker.join();
        } else {
            pending.push(worker);
        }
    }
    *workers = pending;
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(super) fn read_bounded_line(reader: &mut impl BufRead, limit: usize) -> io::Result<InputLine> {
    let mut line = Vec::with_capacity(4096);
    let mut too_long = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if line.is_empty() && !too_long {
                return Ok(InputLine::End);
            }
            return Ok(if too_long {
                InputLine::TooLong
            } else {
                InputLine::Line(line)
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.unwrap_or(available.len());
        if !too_long {
            let remaining = limit.saturating_sub(line.len());
            let accepted = remaining.min(count);
            line.extend_from_slice(&available[..accepted]);
            too_long = accepted < count;
        }
        let consumed = count + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(if too_long {
                InputLine::TooLong
            } else {
                InputLine::Line(line)
            });
        }
    }
}
