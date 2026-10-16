//! `cpal::Stream` is deliberately `!Send` on every platform. To own a stream from
//! application code that lives behind `Arc<Mutex<_>>` (Tauri managed state), the stream
//! is created and dropped on a dedicated thread that we control through a channel.
//! The real-time audio callback itself runs on the host's audio thread, not here.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use cpal::traits::StreamTrait;

use crate::error::{AudioError, Result};

/// Returned by the stream builder: what the stream actually runs at.
#[derive(Debug, Clone, Copy)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

pub(crate) struct StreamThread {
    stop_tx: Option<mpsc::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl StreamThread {
    /// Run `build` on a new thread, start the stream it returns, and keep it alive
    /// until [`StreamThread::stop`] (or drop).
    pub(crate) fn spawn<F>(thread_name: &str, build: F) -> Result<(Self, StreamInfo)>
    where
        F: FnOnce() -> Result<(cpal::Stream, StreamInfo)> + Send + 'static,
    {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<StreamInfo>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let join = thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || {
                let (stream, info) = match build() {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                if let Err(e) = stream.play() {
                    let _ = ready_tx.send(Err(e.into()));
                    return;
                }
                let _ = ready_tx.send(Ok(info));
                // Block until asked to stop (or the sender is dropped).
                let _ = stop_rx.recv();
                drop(stream);
            })
            .map_err(|e| AudioError::Stream(format!("cannot spawn audio thread: {e}")))?;

        let info = ready_rx
            .recv()
            .map_err(|_| AudioError::Stream("audio thread exited before reporting".into()))??;
        Ok((
            Self {
                stop_tx: Some(stop_tx),
                join: Some(join),
            },
            info,
        ))
    }

    /// Stop the stream and wait for the owning thread to finish. Idempotent.
    pub(crate) fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for StreamThread {
    fn drop(&mut self) {
        self.stop();
    }
}
