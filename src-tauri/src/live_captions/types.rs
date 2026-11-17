use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Fr,
}

impl Lang {
    pub const fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Fr,
            Lang::Fr => Lang::En,
        }
    }

    pub const fn english_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "French",
        }
    }
}

/// One utterance of the system audio. Partial updates and the final text share
/// the same `id`; a final with an empty text discards the partial.
#[derive(Serialize, Clone)]
pub struct CaptionSegment {
    pub id: u64,
    pub text: String,
    pub lang: Lang,
    pub is_final: bool,
}

#[derive(Serialize, Clone)]
pub struct CaptionTranslation {
    pub id: u64,
    pub text: String,
}

pub struct TranslationJob {
    pub id: u64,
    pub text: String,
    pub source: Lang,
}

pub struct LiveCaptionsSession {
    pub stop: Arc<AtomicBool>,
    pub worker: JoinHandle<()>,
}

#[derive(Default)]
pub struct LiveCaptionsState {
    pub session: Mutex<Option<LiveCaptionsSession>>,
}
