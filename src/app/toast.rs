use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_TOAST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub title: String,
    pub body: Option<String>,
    pub created: Instant,
    pub life: Duration,
}

impl Toast {
    pub fn age(&self) -> Duration {
        self.created.elapsed()
    }

    pub fn expired(&self) -> bool {
        self.age() >= self.life
    }

    pub fn remaining(&self) -> f32 {
        let life = self.life.as_secs_f32().max(0.001);
        (1.0 - self.age().as_secs_f32() / life).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

const MAX_VISIBLE: usize = 4;

impl Toasts {
    pub fn push(&mut self, kind: ToastKind, title: impl Into<String>, body: Option<String>) {
        let life = match kind {
            ToastKind::Error => Duration::from_secs(12),
            ToastKind::Warning => Duration::from_secs(9),
            _ => Duration::from_secs(5),
        };

        self.items.push(Toast {
            id: NEXT_TOAST_ID.fetch_add(1, Ordering::Relaxed),
            kind,
            title: title.into(),
            body,
            created: Instant::now(),
            life,
        });

        while self.items.len() > MAX_VISIBLE {
            self.items.remove(0);
        }
    }

    pub fn success(&mut self, title: impl Into<String>) {
        self.push(ToastKind::Success, title, None);
    }

    pub fn info(&mut self, title: impl Into<String>) {
        self.push(ToastKind::Info, title, None);
    }

    pub fn warning(&mut self, title: impl Into<String>, body: Option<String>) {
        self.push(ToastKind::Warning, title, body);
    }

    pub fn error(&mut self, title: impl Into<String>, body: Option<String>) {
        self.push(ToastKind::Error, title, body);
    }

    pub fn retire_expired(&mut self) {
        self.items.retain(|toast| !toast.expired());
    }

    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|toast| toast.id != id);
    }

    pub fn items(&self) -> &[Toast] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
