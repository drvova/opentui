use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(not(windows))]
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
#[cfg(not(windows))]
use rustix::io::{Errno, read};
#[cfg(not(windows))]
use rustix::stdio::stdin;

#[derive(Debug)]
pub struct TerminalInputBridge {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    receiver: Receiver<QueuedInputEvent>,
}

#[derive(Debug)]
pub struct QueuedInputEvent {
    pub name: &'static str,
    pub payload: Vec<u8>,
}

impl TerminalInputBridge {
    pub fn start(renderer_ptr: u64) -> Self {
        let (sender, receiver) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let handle = thread::spawn(move || run_input_loop(renderer_ptr, thread_stop, sender));

        Self {
            stop,
            handle: Some(handle),
            receiver,
        }
    }

    pub fn drain(&self, out: &mut VecDeque<QueuedInputEvent>) {
        for event in self.receiver.try_iter() {
            out.push_back(event);
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TerminalInputBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(not(windows))]
fn run_input_loop(renderer_ptr: u64, stop: Arc<AtomicBool>, sender: Sender<QueuedInputEvent>) {
    let stdin_fd = stdin();
    let original_flags = fcntl_getfl(stdin_fd).ok();

    if let Some(flags) = original_flags {
        let _ = fcntl_setfl(stdin_fd, flags | OFlags::NONBLOCK);
    }

    let mut buffer = [0_u8; 4096];

    while !stop.load(Ordering::Relaxed) {
        match read(stdin_fd, &mut buffer[..]) {
            Ok(0) => thread::sleep(Duration::from_millis(8)),
            Ok(count) => {
                let mut payload = Vec::with_capacity(8 + count);
                payload.extend_from_slice(&renderer_ptr.to_le_bytes());
                payload.extend_from_slice(&buffer[..count]);
                let _ = sender.send(QueuedInputEvent {
                    name: "terminal:raw",
                    payload,
                });
            }
            Err(Errno::AGAIN) => thread::sleep(Duration::from_millis(8)),
            Err(_) => thread::sleep(Duration::from_millis(8)),
        }
    }

    if let Some(flags) = original_flags {
        let _ = fcntl_setfl(stdin_fd, flags);
    }
}

#[cfg(windows)]
fn run_input_loop(_renderer_ptr: u64, stop: Arc<AtomicBool>, _sender: Sender<QueuedInputEvent>) {
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(16));
    }
}
