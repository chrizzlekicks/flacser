use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;

pub struct InterruptFlag {
    interrupted: Arc<AtomicBool>,
}

impl InterruptFlag {
    pub fn new() -> Self {
        Self {
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn shared(&self) -> Self {
        Self {
            interrupted: Arc::clone(&self.interrupted),
        }
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

pub fn install_handler(flag: InterruptFlag) -> Result<()> {
    ctrlc::set_handler(move || {
        flag.interrupt();
        eprintln!("Interrupt received; stopping after active jobs...");
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::InterruptFlag;

    #[test]
    fn new_flag_starts_uninterrupted() {
        let flag = InterruptFlag::new();

        assert!(!flag.is_interrupted());
    }

    #[test]
    fn interrupt_marks_flag_interrupted() {
        let flag = InterruptFlag::new();

        flag.interrupt();

        assert!(flag.is_interrupted());
    }

    #[test]
    fn shared_flags_share_state() {
        let flag = InterruptFlag::new();
        let shared = flag.shared();

        shared.interrupt();

        assert!(flag.is_interrupted());
    }
}
