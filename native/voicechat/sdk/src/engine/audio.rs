//! The sound card: the microphone feeds 48 kHz mono samples to the mic
//! thread, and the speakers pull from the mixer. The SDK device backend
//! owns its streams on a dedicated thread.
use crate::engine::mix::Mixer;
use crate::engine::opus::SAMPLE_RATE;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::JoinHandle;

/// Microphone samples waiting for the mic thread; a second at most.
#[derive(Default)]
pub struct MicQueue {
    pub samples: Mutex<VecDeque<f32>>,
    pub ready: Condvar,
}

impl MicQueue {
    pub fn push(&self, samples: &[f32]) {
        let mut q = self.samples.lock().unwrap();
        q.extend(samples);
        let excess = q.len().saturating_sub(SAMPLE_RATE as usize);
        q.drain(..excess);
        drop(q);
        self.ready.notify_one();
    }
}

/// What opened, for the HUD and `/vc status`.
#[derive(Clone, Debug)]
pub struct Devices {
    pub input: Result<String, String>,
    pub output: Result<String, String>,
}

impl Default for Devices {
    fn default() -> Self {
        Self {
            input: Err("not open".into()),
            output: Err("not open".into()),
        }
    }
}

type DeviceBackend = fn(Arc<Mutex<Mixer>>, Arc<MicQueue>, Arc<Mutex<Devices>>, Arc<AtomicBool>);
static BACKEND: OnceLock<DeviceBackend> = OnceLock::new();

pub fn install_backend(run: DeviceBackend) {
    let _ = BACKEND.set(run);
}

pub struct AudioIo {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioIo {
    pub fn start(
        mixer: Arc<Mutex<Mixer>>,
        mic: Arc<MicQueue>,
        devices: Arc<Mutex<Devices>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("voicechat-audio".into())
            .spawn(move || {
                if let Some(run) = BACKEND.get() {
                    run(mixer, mic, devices, flag);
                }
            })
            .ok();
        Self { stop, thread }
    }
}

impl Drop for AudioIo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
