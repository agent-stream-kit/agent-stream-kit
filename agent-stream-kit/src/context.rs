use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::value::AgentValue;

/// Event-scoped context that identifies a single flow across agents and carries auxiliary metadata.
///
/// A context is created per externally triggered event (user input, timer, webhook, etc.) so that
/// agents connected through channels can recognize they are handling the same flow. It can carry
/// auxiliary metadata useful for processing without altering the primary payload.
///
/// When a single datum fans out into multiple derived items (e.g., a `map` operation), frames track
/// the branching lineage. Because mapping can nest, frames behave like a stack to preserve ancestry.
/// Instances are cheap to clone and return new copies instead of mutating in place.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    /// Unique identifier assigned when the context is created.
    id: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    vars: Option<im::HashMap<String, AgentValue>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    frames: Option<im::Vector<Frame>>,
}

impl AgentContext {
    /// Creates a new context with a unique identifier and no state.
    pub fn new() -> Self {
        Self {
            id: new_id(),
            vars: None,
            frames: None,
        }
    }

    /// Returns the unique identifier for this context.
    pub fn id(&self) -> usize {
        self.id
    }

    // Variables

    /// Retrieves an immutable reference to a stored variable, if present.
    pub fn get_var(&self, key: &str) -> Option<&AgentValue> {
        self.vars.as_ref().and_then(|vars| vars.get(key))
    }

    /// Returns a new context with the provided variable inserted while keeping the current context unchanged.
    pub fn with_var(&self, key: String, value: AgentValue) -> Self {
        let mut vars = if let Some(vars) = &self.vars {
            vars.clone()
        } else {
            im::HashMap::new()
        };
        vars.insert(key, value);
        Self {
            id: self.id,
            vars: Some(vars),
            frames: self.frames.clone(),
        }
    }
}

// ID generation
static CONTEXT_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Generates a monotonically increasing identifier for contexts.
fn new_id() -> usize {
    CONTEXT_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// Frame stack

/// Describes a single stack frame captured during agent execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub name: String,
    pub data: AgentValue,
}

impl AgentContext {
    /// Returns the current frame stack, if any frames have been pushed.
    pub fn frames(&self) -> Option<&im::Vector<Frame>> {
        self.frames.as_ref()
    }

    /// Appends a new frame to the end of the stack and returns the updated context.
    pub fn push_frame(&self, name: String, data: AgentValue) -> Self {
        let mut frames = if let Some(frames) = &self.frames {
            frames.clone()
        } else {
            im::Vector::new()
        };
        frames.push_back(Frame { name, data });
        Self {
            id: self.id,
            vars: self.vars.clone(),
            frames: Some(frames),
        }
    }

    /// Removes the most recently pushed frame and returns it together with the updated context.
    /// If the stack is empty, `None` is returned alongside an unchanged context.
    pub fn pop_frame(&self) -> (Option<Frame>, Self) {
        if let Some(frames) = &self.frames {
            if frames.is_empty() {
                return (None, self.clone());
            }
            let mut frames = frames.clone();
            let last = frames.pop_back().unwrap(); // safe unwrap after is_empty check

            let new_frames = if frames.is_empty() {
                None
            } else {
                Some(frames)
            };
            return (
                Some(last),
                Self {
                    id: self.id,
                    vars: self.vars.clone(),
                    frames: new_frames,
                },
            );
        }
        (None, self.clone())
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_assigns_unique_ids() {
        let ctx1 = AgentContext::new();
        let ctx2 = AgentContext::new();

        assert_ne!(ctx1.id(), 0);
        assert_ne!(ctx2.id(), 0);
        assert_ne!(ctx1.id(), ctx2.id());
        assert_eq!(ctx1.id(), ctx1.clone().id());
    }

    #[test]
    fn with_var_sets_value_without_mutating_original() {
        let ctx = AgentContext::new();
        assert!(ctx.get_var("answer").is_none());

        let updated = ctx.with_var("answer".into(), AgentValue::integer(42));

        assert!(ctx.get_var("answer").is_none());
        assert_eq!(updated.get_var("answer"), Some(&AgentValue::integer(42)));
        assert_eq!(ctx.id(), updated.id());
    }

    #[test]
    fn push_and_pop_frames() {
        let ctx = AgentContext::new();
        assert!(ctx.frames().is_none());

        let ctx = ctx
            .push_frame("first".into(), AgentValue::string("a"))
            .push_frame("second".into(), AgentValue::integer(2));

        let frames = ctx.frames().expect("frames should be present");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].name, "first");
        assert_eq!(frames[1].name, "second");
        assert_eq!(frames[1].data, AgentValue::integer(2));

        let (popped_second, ctx) = ctx.pop_frame();
        let popped_second = popped_second.expect("second frame should exist");
        assert_eq!(popped_second.name, "second");
        assert_eq!(ctx.frames().unwrap().len(), 1);
        assert_eq!(ctx.frames().unwrap()[0].name, "first");

        let (popped_first, ctx) = ctx.pop_frame();
        assert_eq!(popped_first.unwrap().name, "first");
        assert!(ctx.frames().is_none());

        let (no_frame, ctx_after_empty) = ctx.pop_frame();
        assert!(no_frame.is_none());
        assert!(ctx_after_empty.frames().is_none());
    }

    #[test]
    fn clone_preserves_vars() {
        let ctx = AgentContext::new().with_var("key".into(), AgentValue::integer(1));
        let cloned = ctx.clone();

        assert_eq!(cloned.get_var("key"), Some(&AgentValue::integer(1)));
        assert_eq!(cloned.id(), ctx.id());
    }

    #[test]
    fn clone_preserves_frames() {
        let ctx = AgentContext::new().push_frame("frame".into(), AgentValue::string("data"));
        let cloned = ctx.clone();

        let frames = cloned.frames().expect("cloned frames should exist");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].name, "frame");
        assert_eq!(frames[0].data, AgentValue::string("data"));
        assert_eq!(cloned.id(), ctx.id());
    }

    #[test]
    fn serialization_skips_empty_optional_fields() {
        let ctx = AgentContext::new();
        let json_ctx = serde_json::to_value(&ctx).unwrap();

        assert!(json_ctx.get("id").and_then(|v| v.as_u64()).is_some());
        assert!(json_ctx.get("vars").is_none());
        assert!(json_ctx.get("frames").is_none());

        let populated = ctx
            .with_var("key".into(), AgentValue::string("value"))
            .push_frame("frame".into(), AgentValue::integer(1));
        let json_populated = serde_json::to_value(&populated).unwrap();

        assert_eq!(json_populated["vars"]["key"], json!("value"));
        let frames = json_populated["frames"]
            .as_array()
            .expect("frames should serialize as array");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["name"], json!("frame"));
        assert_eq!(frames[0]["data"], json!(1));
    }
}
