//! Event emission system for configuration changes.
//!
//! Matches Python prefer's `Emitter` class. Used by `Config` to emit
//! "changed" events when values are set or updated.

use crate::value::ConfigValue;
use std::collections::HashMap;

/// Handler function for configuration events.
///
/// Parameters:
/// - `key`: The configuration key that changed
/// - `value`: The new value
/// - `previous`: The previous value, if any
pub type EventHandler = Box<dyn Fn(&str, &ConfigValue, Option<&ConfigValue>) + Send + Sync>;

/// An event emitter that supports named events with multiple handlers.
pub struct Emitter {
    handlers: HashMap<String, Vec<EventHandler>>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for the given event name.
    pub fn bind(&mut self, event: &str, handler: EventHandler) {
        self.handlers
            .entry(event.to_string())
            .or_default()
            .push(handler);
    }

    /// Emit an event, calling all registered handlers.
    pub fn emit(&self, event: &str, key: &str, value: &ConfigValue, previous: Option<&ConfigValue>) {
        if let Some(handlers) = self.handlers.get(event) {
            for handler in handlers {
                handler(key, value, previous);
            }
        }
    }

    /// Check if any handlers are registered for the given event.
    pub fn has_handlers(&self, event: &str) -> bool {
        self.handlers
            .get(event)
            .is_some_and(|h| !h.is_empty())
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_emit_calls_handlers() {
        let mut emitter = Emitter::new();
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let log_clone = log.clone();
        emitter.bind("changed", Box::new(move |key, _value, _prev| {
            log_clone.lock().unwrap().push(key.to_string());
        }));

        emitter.emit(
            "changed",
            "server.port",
            &ConfigValue::Integer(8080),
            None,
        );

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "server.port");
    }

    #[test]
    fn test_emit_with_previous_value() {
        let mut emitter = Emitter::new();
        let saw_previous: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let flag = saw_previous.clone();
        emitter.bind("changed", Box::new(move |_key, _value, prev| {
            if let Some(ConfigValue::Integer(42)) = prev {
                *flag.lock().unwrap() = true;
            }
        }));

        emitter.emit(
            "changed",
            "port",
            &ConfigValue::Integer(9090),
            Some(&ConfigValue::Integer(42)),
        );

        assert!(*saw_previous.lock().unwrap());
    }

    #[test]
    fn test_emit_no_handlers() {
        let emitter = Emitter::new();
        // Should not panic
        emitter.emit("changed", "key", &ConfigValue::Null, None);
    }

    #[test]
    fn test_multiple_handlers() {
        let mut emitter = Emitter::new();
        let count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

        for _ in 0..3 {
            let c = count.clone();
            emitter.bind("changed", Box::new(move |_, _, _| {
                *c.lock().unwrap() += 1;
            }));
        }

        emitter.emit("changed", "key", &ConfigValue::Null, None);
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn test_has_handlers() {
        let mut emitter = Emitter::new();
        assert!(!emitter.has_handlers("changed"));

        emitter.bind("changed", Box::new(|_, _, _| {}));
        assert!(emitter.has_handlers("changed"));
        assert!(!emitter.has_handlers("other"));
    }
}
