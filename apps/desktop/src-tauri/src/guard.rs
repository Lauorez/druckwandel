use std::sync::{Mutex, MutexGuard, OnceLock};

static DATA: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn exclusive() -> MutexGuard<'static, ()> {
    DATA.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
