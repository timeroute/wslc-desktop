use std::sync::mpsc::{Receiver, TryRecvError};

/// Drain all pending events from a channel; returns `(events, disconnected)`.
pub(crate) fn recv_all<T>(rx: &Receiver<T>) -> (Vec<T>, bool) {
    let mut events = Vec::new();
    let mut disconnected = false;
    loop {
        match rx.try_recv() {
            Ok(event) => events.push(event),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                disconnected = true;
                break;
            }
        }
    }
    (events, disconnected)
}
