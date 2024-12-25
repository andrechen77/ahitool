use std::{
    cell::Cell,
    sync::{
        mpsc::{channel, Receiver, Sender, TryRecvError},
        Mutex, MutexGuard,
    },
};

#[derive(Default)]
pub struct DataLoader<T> {
    /// The most recent data fetched.
    data: Mutex<T>,
    /// If there is a fetch in progress, this holds the receiver for the
    /// response of the fetch.
    rx: Cell<Option<Receiver<T>>>,
}

impl<T> DataLoader<T> {
    #[allow(dead_code)]
    pub fn new(init: T) -> Self {
        Self { data: Mutex::new(init), rx: Cell::new(None) }
    }

    pub fn get_mut(&self) -> MutexGuard<'_, T> {
        if let Some(rx) = self.rx.take() {
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
                Err(TryRecvError::Disconnected) => {
                    // do not put the receive back since there is no chance it
                    // will produce a value
                }
            }
        }
        self.data.lock().unwrap()
    }

    pub fn fetch_in_progress(&self) -> bool {
        if let Some(rx) = self.rx.take() {
            match rx.try_recv() {
                Ok(new_data) => {
                    *self.data.lock().unwrap() = new_data;
                    false
                }
                Err(TryRecvError::Empty) => {
                    self.rx.set(Some(rx));
                    true
                }
                Err(TryRecvError::Disconnected) => false,
            }
        } else {
            false
        }
    }

    /// Marks the `DataLoader` as having a fetch in progress, returning the
    /// sender that the fetching task should use to report the response.
    #[must_use]
    pub fn start_fetch(&mut self) -> Sender<T> {
        let (tx, rx) = channel();
        self.rx.set(Some(rx));
        tx
    }
}
