//! Non-blocking side-band client stub (W20-09). Transport-agnostic: C5 (gateway)
//! and wire format remain open, so the transport is a trait. All Hermes I/O runs
//! on a dedicated side thread; the 80 ms hot path only ever calls `submit`
//! (bounded `try_send`, never blocks) and `poll` (`try_recv`, never blocks).
use crate::{within_cap, FromHermes, NoKnowledgeReason, ToHermes, TurnId};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::Instant;

/// Pluggable transport to the owner's installed Hermes. Called on the side thread only.
pub trait HermesTransport: Send + 'static {
    /// Deliver an event; may return a reply. May block — it is off the hot path.
    fn exchange(&mut self, ev: ToHermes) -> Option<FromHermes>;
}

/// Offline transport: every lookup answers `NoKnowledge::Unreachable` (honest filler).
pub struct NullTransport;
impl HermesTransport for NullTransport {
    fn exchange(&mut self, ev: ToHermes) -> Option<FromHermes> {
        match ev {
            ToHermes::Intent { turn, .. } => Some(FromHermes::NoKnowledge { turn, reason: NoKnowledgeReason::Unreachable }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError { QueueFull, Closed }

pub struct SidebandClient {
    tx: Option<SyncSender<ToHermes>>,
    rx: Receiver<FromHermes>,
    pending: HashMap<TurnId, Instant>,
    ready: Vec<FromHermes>,
    worker: Option<JoinHandle<()>>,
}

impl SidebandClient {
    /// Spawn the side thread. `queue` bounds outstanding events (back-pressure = drop, never wait).
    pub fn spawn<T: HermesTransport>(mut transport: T, queue: usize) -> Self {
        let (tx, wrx) = mpsc::sync_channel::<ToHermes>(queue.max(1));
        let (wtx, rx) = mpsc::channel::<FromHermes>();
        let worker = std::thread::Builder::new()
            .name("hermes-sideband".into())
            .spawn(move || {
                for ev in wrx {
                    if let Some(r) = transport.exchange(ev) {
                        if wtx.send(r).is_err() { break; }
                    }
                }
            })
            .expect("spawn sideband thread");
        Self { tx: Some(tx), rx, pending: HashMap::new(), ready: Vec::new(), worker: Some(worker) }
    }

    /// Hot-path safe: never blocks. Intents start the 3 s retrieval clock.
    pub fn submit(&mut self, ev: ToHermes) -> Result<(), SubmitError> {
        let turn = if let ToHermes::Intent { turn, .. } = &ev { Some(*turn) } else { None };
        let tx = self.tx.as_ref().ok_or(SubmitError::Closed)?;
        match tx.try_send(ev) {
            Ok(()) => { if let Some(t) = turn { self.pending.insert(t, Instant::now()); } Ok(()) }
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Closed),
        }
    }

    /// Hot-path safe: never blocks. Returns a reply for `turn`, a `Timeout`
    /// NoKnowledge once past `RETRIEVAL_CAP`, or `None` (keep talking).
    pub fn poll(&mut self, turn: TurnId) -> Option<FromHermes> {
        while let Ok(r) = self.rx.try_recv() { self.ready.push(r); }
        let started = *self.pending.get(&turn)?;
        let expired = !within_cap(started.elapsed());
        if expired {
            self.pending.remove(&turn);
            self.ready.retain(|r| reply_turn(r) != turn); // late replies dropped
            return Some(FromHermes::NoKnowledge { turn, reason: NoKnowledgeReason::Timeout });
        }
        let i = self.ready.iter().position(|r| reply_turn(r) == turn)?;
        self.pending.remove(&turn);
        Some(self.ready.swap_remove(i))
    }
}

fn reply_turn(r: &FromHermes) -> TurnId {
    match r { FromHermes::Knowledge { turn, .. } | FromHermes::NoKnowledge { turn, .. } => *turn }
}

impl Drop for SidebandClient {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(w) = self.worker.take() { let _ = w.join(); }
    }
}
