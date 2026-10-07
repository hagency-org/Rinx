//! Recheck a caller's authority after the document picker returns, before IO.
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock}};

pub(crate) type Authorizer = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Default)]
struct Authorizers(Mutex<HashMap<usize, Authorizer>>);

impl Authorizers {
    fn insert(&self, callback: usize, authorize: Authorizer) {
        // A poisoned registry must never silently turn a guarded save into an
        // ordinary save. Recover the storage; authorization still fails closed.
        self.0.lock().unwrap_or_else(|error| error.into_inner()).insert(callback, authorize);
    }

    fn remove(&self, callback: usize) {
        self.0.lock().unwrap_or_else(|error| error.into_inner()).remove(&callback);
    }

    fn check(&self, callback: usize) -> bool {
        let Ok(guards) = self.0.lock() else { return false };
        let authorize = guards.get(&callback).cloned();
        drop(guards);
        // Existing unguarded picker operations retain their original behavior.
        authorize.is_none_or(|authorize| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| authorize()))
                .unwrap_or(false)
        })
    }
}

fn registry() -> &'static Authorizers {
    static REGISTRY: OnceLock<Authorizers> = OnceLock::new();
    REGISTRY.get_or_init(Authorizers::default)
}

pub(crate) fn insert(callback: usize, authorize: Authorizer) { registry().insert(callback, authorize); }
pub(crate) fn remove(callback: usize) { registry().remove(callback); }
pub(crate) fn check(callback: usize) -> bool { registry().check(callback) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn checks_current_authority_after_picker_wait_and_releases_on_cancel() {
        let guards = Authorizers::default();
        let valid = Arc::new(AtomicBool::new(true));
        let current = valid.clone();
        guards.insert(1, Arc::new(move || current.load(Ordering::SeqCst)));
        assert!(guards.check(1));
        valid.store(false, Ordering::SeqCst);
        assert!(!guards.check(1));
        guards.remove(1);
        assert_eq!(Arc::strong_count(&valid), 1);
        assert!(guards.check(2));
    }

    #[test]
    fn panic_does_not_cross_jni_or_authorize_a_write() {
        let guards = Authorizers::default();
        guards.insert(1, Arc::new(|| panic!("revoked")));
        assert!(!guards.check(1));
    }
}
