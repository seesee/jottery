use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const KEY_LENGTH: usize = 32;

/// Master key stored in memory
/// Never persisted to disk
#[derive(Clone)]
pub struct MasterKey {
    key: [u8; KEY_LENGTH],
    derived_at: Instant,
}

impl MasterKey {
    /// Create a new master key
    pub fn new(key: [u8; KEY_LENGTH]) -> Self {
        Self {
            key,
            derived_at: Instant::now(),
        }
    }

    /// Get the key bytes
    pub fn key(&self) -> &[u8; KEY_LENGTH] {
        &self.key
    }

    /// Get the time when the key was derived
    pub fn derived_at(&self) -> Instant {
        self.derived_at
    }

    /// Get the age of the key
    pub fn age(&self) -> Duration {
        self.derived_at.elapsed()
    }
}

/// Key manager for in-memory master key storage
/// Handles auto-lock functionality
#[derive(Clone)]
pub struct KeyManager {
    inner: Arc<Mutex<KeyManagerInner>>,
}

struct KeyManagerInner {
    master_key: Option<MasterKey>,
    last_activity: Instant,
    auto_lock_duration: Option<Duration>,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(KeyManagerInner {
                master_key: None,
                last_activity: Instant::now(),
                auto_lock_duration: None,
            })),
        }
    }

    /// Get the current master key (if unlocked)
    pub fn get_master_key(&self) -> Option<MasterKey> {
        let mut inner = self.inner.lock().unwrap();

        // Check auto-lock
        if let Some(duration) = inner.auto_lock_duration {
            if inner.last_activity.elapsed() > duration {
                // Auto-lock triggered
                inner.master_key = None;
                return None;
            }
        }

        inner.master_key.clone()
    }

    /// Set the master key (on unlock)
    pub fn set_master_key(&self, key: [u8; KEY_LENGTH]) {
        let mut inner = self.inner.lock().unwrap();
        inner.master_key = Some(MasterKey::new(key));
        inner.last_activity = Instant::now();
    }

    /// Clear the master key (on lock)
    /// Overwrites the key with zeros before dropping
    pub fn clear_master_key(&self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(ref mut key) = inner.master_key {
            // Overwrite key with zeros for security
            key.key = [0u8; KEY_LENGTH];
        }
        inner.master_key = None;
    }

    /// Check if the application is locked
    pub fn is_locked(&self) -> bool {
        self.get_master_key().is_none()
    }

    /// Register user activity (resets auto-lock timer)
    pub fn register_activity(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.last_activity = Instant::now();
    }

    /// Set auto-lock timeout
    ///
    /// # Arguments
    /// * `timeout_minutes` - Timeout in minutes (0 to disable)
    pub fn set_auto_lock(&self, timeout_minutes: u64) {
        let mut inner = self.inner.lock().unwrap();
        if timeout_minutes == 0 {
            inner.auto_lock_duration = None;
        } else {
            inner.auto_lock_duration = Some(Duration::from_secs(timeout_minutes * 60));
        }
    }

    /// Get auto-lock timeout in minutes
    pub fn get_auto_lock_minutes(&self) -> Option<u64> {
        let inner = self.inner.lock().unwrap();
        inner.auto_lock_duration.map(|d| d.as_secs() / 60)
    }

    /// Get time since last activity
    pub fn time_since_activity(&self) -> Duration {
        let inner = self.inner.lock().unwrap();
        inner.last_activity.elapsed()
    }

    /// Get time until auto-lock (if enabled)
    pub fn time_until_lock(&self) -> Option<Duration> {
        let inner = self.inner.lock().unwrap();
        if let Some(duration) = inner.auto_lock_duration {
            let elapsed = inner.last_activity.elapsed();
            if elapsed < duration {
                Some(duration - elapsed)
            } else {
                Some(Duration::ZERO)
            }
        } else {
            None
        }
    }

    /// Check if auto-lock should trigger
    pub fn should_lock(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if let Some(duration) = inner.auto_lock_duration {
            inner.last_activity.elapsed() > duration && inner.master_key.is_some()
        } else {
            false
        }
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_key_manager_lock_unlock() {
        let manager = KeyManager::new();
        assert!(manager.is_locked());

        let key = [1u8; KEY_LENGTH];
        manager.set_master_key(key);
        assert!(!manager.is_locked());

        let retrieved = manager.get_master_key().unwrap();
        assert_eq!(retrieved.key(), &key);

        manager.clear_master_key();
        assert!(manager.is_locked());
    }

    #[test]
    fn test_auto_lock() {
        let manager = KeyManager::new();
        manager.set_auto_lock(0); // 0 = disabled

        let key = [1u8; KEY_LENGTH];
        manager.set_master_key(key);
        assert!(!manager.is_locked());

        // With auto-lock disabled, should not lock
        thread::sleep(Duration::from_millis(100));
        assert!(!manager.is_locked());

        // Enable auto-lock with very short timeout (for testing)
        manager.set_auto_lock(0); // Reset
        {
            let mut inner = manager.inner.lock().unwrap();
            inner.auto_lock_duration = Some(Duration::from_millis(100));
        }

        // Should lock after timeout
        thread::sleep(Duration::from_millis(150));
        assert!(manager.is_locked());
    }

    #[test]
    fn test_register_activity() {
        let manager = KeyManager::new();
        {
            let mut inner = manager.inner.lock().unwrap();
            inner.auto_lock_duration = Some(Duration::from_millis(100));
        }

        let key = [1u8; KEY_LENGTH];
        manager.set_master_key(key);

        // Register activity before timeout
        thread::sleep(Duration::from_millis(50));
        manager.register_activity();
        thread::sleep(Duration::from_millis(50));

        // Should not be locked (activity was registered)
        assert!(!manager.is_locked());

        // Wait for timeout without activity
        thread::sleep(Duration::from_millis(150));
        assert!(manager.is_locked());
    }

    #[test]
    fn test_time_until_lock() {
        let manager = KeyManager::new();
        manager.set_auto_lock(1); // 1 minute

        let key = [1u8; KEY_LENGTH];
        manager.set_master_key(key);

        let time_left = manager.time_until_lock().unwrap();
        assert!(time_left.as_secs() <= 60);
        assert!(time_left.as_secs() > 55); // Should be close to 60 seconds
    }

    #[test]
    fn test_should_lock() {
        let manager = KeyManager::new();
        {
            let mut inner = manager.inner.lock().unwrap();
            inner.auto_lock_duration = Some(Duration::from_millis(50));
        }

        let key = [1u8; KEY_LENGTH];
        manager.set_master_key(key);

        assert!(!manager.should_lock());

        thread::sleep(Duration::from_millis(100));
        assert!(manager.should_lock());
    }
}
