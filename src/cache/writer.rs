use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

pub enum WriterCommand {
    Write { path: PathBuf, content: String },
    Delete { path: PathBuf },
    Shutdown,
}

pub struct CacheWriter {
    tx: Sender<WriterCommand>,
    handle: Option<JoinHandle<()>>,
}

impl CacheWriter {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || writer_loop(rx));
        Self { tx, handle: Some(handle) }
    }

    pub fn write(&self, path: PathBuf, content: String) {
        let _ = self.tx.send(WriterCommand::Write { path, content });
    }

    pub fn delete(&self, path: PathBuf) {
        let _ = self.tx.send(WriterCommand::Delete { path });
    }

    /// Sends `Shutdown`, then blocks until the thread finishes all queued work.
    pub fn shutdown(mut self) {
        let _ = self.tx.send(WriterCommand::Shutdown);
        // Drop the sender so the channel closes after Shutdown is processed.
        drop(self.tx);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn atomic_write(path: &PathBuf, content: &str) {
    let tmp = path.with_extension("tmp");
    match fs::write(&tmp, content.as_bytes()) {
        Ok(()) => {
            if let Err(e) = atomic_rename(&tmp, path) {
                log::error!("Failed to rename {:?} → {:?}: {}", tmp, path, e);
                let _ = fs::remove_file(&tmp);
            }
        }
        Err(e) => log::error!("Failed to write {:?}: {}", tmp, e),
    }
}

#[cfg(unix)]
fn atomic_rename(from: &PathBuf, to: &PathBuf) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(not(unix))]
fn atomic_rename(from: &PathBuf, to: &PathBuf) -> std::io::Result<()> {
    // Windows: rename fails if target exists; remove first (not truly atomic).
    let _ = fs::remove_file(to);
    fs::rename(from, to)
}

fn writer_loop(rx: Receiver<WriterCommand>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WriterCommand::Write { path, content } => atomic_write(&path, &content),
            WriterCommand::Delete { path } => {
                if let Err(e) = fs::remove_file(&path) {
                    log::warn!("Failed to delete {:?}: {}", path, e);
                }
            }
            WriterCommand::Shutdown => break,
        }
    }
}
