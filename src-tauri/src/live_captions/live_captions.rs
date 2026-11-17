use super::helpers::{detect_language, Segmenter};
use super::translation::spawn_translator;
use super::types::{CaptionSegment, Lang, LiveCaptionsSession, LiveCaptionsState, TranslationJob};
use crate::audio::pipeline::{process_chunk, ChunkOutcome};
use log::{debug, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// How long to wait for audio before counting the gap as silence: WASAPI
/// loopback delivers nothing while no application is playing sound.
const RECV_TIMEOUT: Duration = Duration::from_millis(100);
/// Minimum pause between two provisional transcriptions of the same utterance.
const PARTIAL_INTERVAL: Duration = Duration::from_millis(700);
/// Shortest utterance worth a provisional transcription: below one second
/// the model guesses a stray word ("to", "okay") that flickers on screen.
const PARTIAL_MIN_MS: usize = 1000;

pub fn start(app: &AppHandle, model: Option<String>) -> Result<(), String> {
    let state = app.state::<LiveCaptionsState>();
    let mut session = state.session.lock();
    if session.is_some() {
        return Ok(());
    }
    crate::audio::preload_engine(app).map_err(|e| format!("{:#}", e))?;

    let stop = Arc::new(AtomicBool::new(false));
    let translator = model
        .filter(|name| !name.trim().is_empty())
        .map(|name| spawn_translator(app, name, stop.clone()));

    // The capture stream is not Send: the worker opens it and reports back.
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let worker = std::thread::Builder::new()
        .name("live-captions".into())
        .spawn({
            let app = app.clone();
            let stop = stop.clone();
            move || run_session(app, stop, translator, ready_tx)
        })
        .map_err(|e| format!("Failed to spawn live captions thread: {}", e))?;

    match ready_rx.recv() {
        Ok(Ok(())) => {
            *session = Some(LiveCaptionsSession { stop, worker });
            info!("Live captions started");
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Live captions stopped before the capture started".to_string()),
    }
}

pub fn stop(app: &AppHandle) {
    let session = app.state::<LiveCaptionsState>().session.lock().take();
    if let Some(session) = session {
        session.stop.store(true, Ordering::SeqCst);
        let _ = session.worker.join();
        info!("Live captions stopped");
    }
}

pub fn is_running(app: &AppHandle) -> bool {
    app.state::<LiveCaptionsState>().session.lock().is_some()
}

#[cfg(target_os = "windows")]
fn open_system_audio(tx: Sender<Vec<f32>>) -> Result<(cpal::Stream, u32), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    // WASAPI turns an input stream opened on a render device into a loopback
    // capture of everything that device plays.
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "No audio output device available".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("No output config available: {}", e))?;
    let stream = crate::audio::recorder::build_mono_input_stream(&device, &config, tx)
        .map_err(|e| format!("Failed to capture the system audio: {:#}", e))?;
    stream
        .play()
        .map_err(|e| format!("Failed to start the system audio capture: {}", e))?;
    Ok((stream, config.sample_rate()))
}

// shortcut: system audio capture is Windows-only (WASAPI loopback), add a
// PipeWire monitor source or ScreenCaptureKit when Linux or macOS need it.
#[cfg(not(target_os = "windows"))]
fn open_system_audio(_tx: Sender<Vec<f32>>) -> Result<(cpal::Stream, u32), String> {
    Err("Live captions are only available on Windows for now".to_string())
}

/// Runs a whole session: capture, cut into utterances, transcribe, queue for
/// translation. Transcription always runs first; translation only ever
/// receives finished utterances and works on its own thread.
fn run_session(
    app: AppHandle,
    stop: Arc<AtomicBool>,
    translator: Option<Sender<TranslationJob>>,
    ready_tx: Sender<Result<(), String>>,
) {
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let (stream, sample_rate) = match open_system_audio(tx) {
        Ok(opened) => opened,
        Err(e) => {
            let _ = ready_tx.send(Err(e));
            return;
        }
    };
    let _ = ready_tx.send(Ok(()));

    let gap = vec![0.0; sample_rate as usize * RECV_TIMEOUT.as_millis() as usize / 1000];
    let mut segmenter = Segmenter::new(sample_rate);
    let mut captions = CaptionEmitter {
        app,
        translator,
        sample_rate,
        next_id: 0,
        lang: Lang::En,
        partial_shown: false,
    };
    let partial_min_len = sample_rate as usize * PARTIAL_MIN_MS / 1000;
    let mut partial_len = 0;
    let mut last_partial = Instant::now();

    while !stop.load(Ordering::SeqCst) {
        let mut finished = Vec::new();
        match rx.recv_timeout(RECV_TIMEOUT) {
            Ok(mono) => {
                finished.extend(segmenter.push(&mono));
                // Catch up on everything captured during the last inference.
                for mono in rx.try_iter() {
                    finished.extend(segmenter.push(&mono));
                }
            }
            Err(RecvTimeoutError::Timeout) => finished.extend(segmenter.push(&gap)),
            Err(RecvTimeoutError::Disconnected) => break,
        }

        for samples in finished {
            captions.finish(samples);
            partial_len = 0;
        }

        if last_partial.elapsed() < PARTIAL_INTERVAL {
            continue;
        }
        if let Some(samples) = segmenter.speech_in_progress() {
            if samples.len() >= partial_min_len && samples.len() != partial_len {
                partial_len = samples.len();
                captions.preview(samples.to_vec());
                last_partial = Instant::now();
            }
        }
    }

    drop(stream);
    if let Some(samples) = segmenter.flush() {
        captions.finish(samples);
    }
    debug!("Live captions session ended");
}

struct CaptionEmitter {
    app: AppHandle,
    translator: Option<Sender<TranslationJob>>,
    sample_rate: u32,
    /// Id of the utterance in progress.
    next_id: u64,
    /// Language of the last utterance, the tie-breaker of the detection.
    lang: Lang,
    /// A provisional text of the utterance in progress is on screen.
    partial_shown: bool,
}

impl CaptionEmitter {
    fn transcribe(&self, samples: Vec<f32>) -> Option<String> {
        match process_chunk(&self.app, samples, self.sample_rate) {
            ChunkOutcome::Text { corrected, .. } => Some(corrected),
            ChunkOutcome::Empty | ChunkOutcome::Failed => None,
        }
    }

    fn emit(&self, text: String, lang: Lang, is_final: bool) {
        let payload = CaptionSegment {
            id: self.next_id,
            text,
            lang,
            is_final,
        };
        let _ = self.app.emit("live-captions-segment", payload);
    }

    /// Provisional transcription of the utterance in progress.
    fn preview(&mut self, samples: Vec<f32>) {
        if let Some(text) = self.transcribe(samples) {
            let lang = detect_language(&text, self.lang);
            self.emit(text, lang, false);
            self.partial_shown = true;
        }
    }

    /// Final transcription of a finished utterance, then off to translation.
    fn finish(&mut self, samples: Vec<f32>) {
        match self.transcribe(samples) {
            Some(text) => {
                self.lang = detect_language(&text, self.lang);
                self.emit(text.clone(), self.lang, true);
                if let Some(translator) = self.translator.as_ref() {
                    let _ = translator.send(TranslationJob {
                        id: self.next_id,
                        text,
                        source: self.lang,
                    });
                }
            }
            // Nothing intelligible after all: an empty final discards the partial.
            None if self.partial_shown => self.emit(String::new(), self.lang, true),
            None => {}
        }
        self.next_id += 1;
        self.partial_shown = false;
    }
}
