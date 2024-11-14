use std::{
    cell::Cell,
    sync::{Mutex, MutexGuard},
};

use tokio::sync::oneshot::{self, error::TryRecvError};

#[derive(Default)]
pub struct DataLoader<T> {
    /// The most recent data fetched.
    data: Mutex<T>,
    /// If there is a fetch in progress, this holds the receiver for the
    /// response of the fetch.
    rx: Cell<Option<oneshot::Receiver<T>>>,
}

impl<T> DataLoader<T> {
    #[allow(dead_code)]
    pub fn new(init: T) -> Self {
        Self { data: Mutex::new(init), rx: Cell::new(None) }
    }

    pub fn get_mut(&self) -> MutexGuard<'_, T> {
        if let Some(mut rx) = self.rx.take() {
            match rx.try_recv() {
                Ok(new_data) => {
                    let mut guard = self.data.lock().unwrap();
                    *guard = new_data;
                    return guard;
                }
                Err(TryRecvError::Empty) => {
                    // put the receiver back since it might still produce a
                    // value
                    self.rx.set(Some(rx));
                }
                Err(TryRecvError::Closed) => {
                    // do not put the receive back since there is no chance it
                    // will produce a value
                }
            }
        }
        self.data.lock().unwrap()
    }

    pub fn fetch_in_progress(&self) -> bool {
        let current = self.rx.take();
        let fetch_in_progress = current.is_some();
        self.rx.set(current);
        fetch_in_progress
    }

    /// Marks the `DataLoader` as having a fetch in progress, returning the
    /// sender that the fetching task should use to report the response.
    #[must_use]
    pub fn start_fetch(&mut self) -> oneshot::Sender<T> {
        let (tx, rx) = oneshot::channel();
        self.rx.set(Some(rx));
        tx
    }
}
