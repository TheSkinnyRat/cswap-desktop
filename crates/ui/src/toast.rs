//! Transient messages (bottom of the window, ~4 s, errors stay until dismissed).
//! The queue is the contract; drawing it belongs to ui-shell.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    /// `ctx.input(|i| i.time)` when it was pushed; 0.0 until the shell stamps it.
    pub at: f64,
}

#[derive(Debug, Default)]
pub struct Toasts {
    pub items: Vec<Toast>,
}

impl Toasts {
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.items.push(Toast { kind, text: text.into(), at: 0.0 });
    }
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Info, text)
    }
    pub fn success(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Success, text)
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Error, text)
    }
}
