use super::types::Lang;
use crate::audio::helpers::rms;
use crate::audio::vad::{AdaptiveVad, VoiceActivity};
use std::cmp::Ordering;

// Function words that exist in one language only: words shared by both
// ("a", "on", "or", "me", "plus", "as", "an", "son"...) are left out on purpose.
const FRENCH_WORDS: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "du", "de", "et", "est", "que", "qui", "dans", "pour",
    "pas", "sur", "avec", "ce", "cette", "ces", "il", "elle", "nous", "vous", "ils", "elles", "je",
    "tu", "au", "aux", "ne", "se", "sa", "ses", "mais", "ou", "donc", "sont", "ont", "mon", "ma",
    "mes", "comme", "tout", "bien", "oui", "non", "alors", "parce", "leur", "en", "y", "suis",
    "avons", "avez", "fait", "faire", "peut", "cela", "merci", "beaucoup", "bonjour", "bonsoir",
    "salut", "aussi", "sommes", "quoi", "pourquoi", "comment", "quand", "chez", "sans", "sous",
    "vers", "encore", "toujours", "jamais", "rien", "peu", "trop", "ici",
];
const ENGLISH_WORDS: &[&str] = &[
    "the", "and", "is", "are", "was", "were", "of", "to", "in", "that", "it", "for", "with",
    "this", "these", "those", "be", "have", "has", "had", "not", "so", "very", "you", "he", "she",
    "we", "they", "i", "my", "your", "his", "her", "our", "their", "at", "by", "from", "what",
    "which", "who", "will", "would", "can", "could", "do", "does", "did", "there", "here", "just",
    "about", "if", "because", "yes", "no", "when", "how", "all", "been", "more", "one", "hello",
    "thanks", "thank", "please", "good", "welcome", "sorry", "well", "now", "then", "also", "some",
    "any", "into", "over", "them", "its", "than", "too", "out", "up", "get", "got", "like", "know",
    "think", "going", "want", "need", "right", "really",
];
// "l'heure", "qu'il": the part before the apostrophe.
const FRENCH_ELISIONS: &[&str] = &["c", "d", "j", "l", "m", "n", "s", "t", "qu"];
// "it's", "don't": the part after the apostrophe.
const ENGLISH_CONTRACTIONS: &[&str] = &["s", "t", "re", "ve", "ll", "m", "d"];
const FRENCH_ACCENTS: &str = "àâçéèêëîïôùûœ";

/// Guesses whether `text` is English or French from function words, elisions
/// and accents. `fallback` (the previous utterance's language) wins on a tie.
pub(super) fn detect_language(text: &str, fallback: Lang) -> Lang {
    let mut french = 0usize;
    let mut english = 0usize;
    let lowered = text.to_lowercase().replace('’', "'");

    for word in lowered.split(|c: char| !(c.is_alphabetic() || c == '\'')) {
        if word.is_empty() {
            continue;
        }
        if word.chars().any(|c| FRENCH_ACCENTS.contains(c)) {
            french += 1;
        }
        match word.split_once('\'') {
            Some((head, _)) if FRENCH_ELISIONS.contains(&head) => french += 1,
            Some((_, tail)) if ENGLISH_CONTRACTIONS.contains(&tail) => english += 1,
            _ if FRENCH_WORDS.contains(&word) => french += 1,
            _ if ENGLISH_WORDS.contains(&word) => english += 1,
            _ => {}
        }
    }

    match french.cmp(&english) {
        Ordering::Greater => Lang::Fr,
        Ordering::Less => Lang::En,
        Ordering::Equal => fallback,
    }
}

/// VAD cadence, same as the recorder's throttle tick the VAD is tuned for.
const TICK_MS: usize = 33;
/// Audio kept before the VAD fires so the first syllable is not clipped.
const PRE_ROLL_MS: usize = 300;
/// Detected silence that ends an utterance. The VAD itself needs ~300 ms of
/// quiet before reporting silence, so the real pause is about 700 ms.
const SILENCE_CUT_MS: usize = 400;
/// Past this length the first detected pause ends the utterance.
const SOFT_LIMIT_SECS: usize = 8;
const SOFT_SILENCE_CUT_MS: usize = 66;
/// shortcut: hard cut without overlap can split a word when someone speaks
/// 15 s without any pause, reuse the chunker's overlap dedup if that shows up.
const HARD_LIMIT_SECS: usize = 15;

/// Cuts a continuous mono stream into utterances: one per pause, with a
/// length cap so captions keep flowing during long monologues.
pub(super) struct Segmenter {
    tick_samples: usize,
    pre_roll_samples: usize,
    silence_cut_samples: usize,
    soft_limit_samples: usize,
    soft_silence_cut_samples: usize,
    hard_limit_samples: usize,
    /// Input not yet grouped into a full tick.
    pending: Vec<f32>,
    /// Pre-roll while waiting for speech, then the utterance in progress.
    samples: Vec<f32>,
    vad: AdaptiveVad,
    in_speech: bool,
    silence_run: usize,
}

impl Segmenter {
    pub(super) fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as usize;
        Self {
            tick_samples: (sr * TICK_MS / 1000).max(1),
            pre_roll_samples: sr * PRE_ROLL_MS / 1000,
            silence_cut_samples: sr * SILENCE_CUT_MS / 1000,
            soft_limit_samples: sr * SOFT_LIMIT_SECS,
            soft_silence_cut_samples: sr * SOFT_SILENCE_CUT_MS / 1000,
            hard_limit_samples: sr * HARD_LIMIT_SECS,
            pending: Vec::new(),
            samples: Vec::new(),
            vad: AdaptiveVad::new(),
            in_speech: false,
            silence_run: 0,
        }
    }

    /// Feeds mono samples and returns the utterances they complete.
    pub(super) fn push(&mut self, mono: &[f32]) -> Vec<Vec<f32>> {
        self.pending.extend_from_slice(mono);
        let pending = std::mem::take(&mut self.pending);
        let ticks = pending.chunks_exact(self.tick_samples);
        self.pending = ticks.remainder().to_vec();
        ticks.filter_map(|tick| self.on_tick(tick)).collect()
    }

    /// The utterance in progress, for a provisional transcription.
    pub(super) fn speech_in_progress(&self) -> Option<&[f32]> {
        self.in_speech.then_some(self.samples.as_slice())
    }

    /// Ends the stream: returns the utterance in progress, if any.
    pub(super) fn flush(&mut self) -> Option<Vec<f32>> {
        self.in_speech.then(|| self.cut())
    }

    fn on_tick(&mut self, tick: &[f32]) -> Option<Vec<f32>> {
        let activity = self.vad.update(rms(tick));
        self.samples.extend_from_slice(tick);

        if !self.in_speech {
            match activity {
                VoiceActivity::Active => self.in_speech = true,
                VoiceActivity::Silent | VoiceActivity::NotStarted => {
                    let excess = self.samples.len().saturating_sub(self.pre_roll_samples);
                    self.samples.drain(..excess);
                }
            }
            return None;
        }

        match activity {
            VoiceActivity::Active => self.silence_run = 0,
            VoiceActivity::Silent | VoiceActivity::NotStarted => self.silence_run += tick.len(),
        }

        let length = self.samples.len();
        let silence_needed = if length >= self.soft_limit_samples {
            self.soft_silence_cut_samples
        } else {
            self.silence_cut_samples
        };
        if self.silence_run >= silence_needed || length >= self.hard_limit_samples {
            return Some(self.cut());
        }
        None
    }

    fn cut(&mut self) -> Vec<f32> {
        self.in_speech = false;
        self.silence_run = 0;
        self.vad.reset_speech_state();
        std::mem::take(&mut self.samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 16000;

    fn speech(secs: f32) -> Vec<f32> {
        vec![0.1; (SR as f32 * secs) as usize]
    }

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (SR as f32 * secs) as usize]
    }

    /// Feeds the audio in uneven buffers, like a capture callback would.
    fn feed(segmenter: &mut Segmenter, samples: &[f32]) -> Vec<Vec<f32>> {
        samples
            .chunks(700)
            .flat_map(|buffer| segmenter.push(buffer))
            .collect()
    }

    fn secs(samples: &[f32]) -> f32 {
        samples.len() as f32 / SR as f32
    }

    #[test]
    fn detects_english() {
        let text = "The weather is nice today and we are going to the park.";
        assert_eq!(detect_language(text, Lang::Fr), Lang::En);
        assert_eq!(
            detect_language("I don't think it's ready", Lang::Fr),
            Lang::En
        );
    }

    #[test]
    fn detects_french() {
        let text = "Bonjour, je voudrais réserver une table pour deux personnes.";
        assert_eq!(detect_language(text, Lang::En), Lang::Fr);
        assert_eq!(
            detect_language("C’est l'heure d'y aller", Lang::En),
            Lang::Fr
        );
    }

    #[test]
    fn detects_short_utterances() {
        assert_eq!(detect_language("Merci beaucoup.", Lang::En), Lang::Fr);
        assert_eq!(detect_language("Thank you.", Lang::Fr), Lang::En);
    }

    #[test]
    fn ambiguous_text_keeps_the_previous_language() {
        assert_eq!(detect_language("OK", Lang::Fr), Lang::Fr);
        assert_eq!(detect_language("OK", Lang::En), Lang::En);
    }

    #[test]
    fn silence_produces_no_utterance() {
        let mut segmenter = Segmenter::new(SR);
        assert!(feed(&mut segmenter, &silence(5.0)).is_empty());
        assert!(segmenter.speech_in_progress().is_none());
        assert!(segmenter.flush().is_none());
        assert!(segmenter.samples.len() <= segmenter.pre_roll_samples);
    }

    #[test]
    fn a_pause_ends_the_utterance() {
        let mut segmenter = Segmenter::new(SR);
        let mut audio = speech(2.0);
        audio.extend(silence(1.5));

        let utterances = feed(&mut segmenter, &audio);

        assert_eq!(utterances.len(), 1);
        assert!((2.0..3.2).contains(&secs(&utterances[0])));
        assert!(segmenter.speech_in_progress().is_none());
    }

    #[test]
    fn a_short_pause_does_not_split_a_short_utterance() {
        let mut segmenter = Segmenter::new(SR);
        let mut audio = speech(2.0);
        audio.extend(silence(0.5));
        audio.extend(speech(1.0));

        assert!(feed(&mut segmenter, &audio).is_empty());
        assert!(segmenter.speech_in_progress().is_some());
    }

    #[test]
    fn a_long_utterance_is_cut_at_its_first_pause() {
        let mut segmenter = Segmenter::new(SR);
        let mut audio = speech(9.0);
        audio.extend(silence(0.5));
        audio.extend(speech(1.0));

        let utterances = feed(&mut segmenter, &audio);

        assert_eq!(utterances.len(), 1);
        assert!((9.0..9.6).contains(&secs(&utterances[0])));
        assert!(segmenter.speech_in_progress().is_some());
    }

    #[test]
    fn speech_without_any_pause_is_force_cut() {
        let mut segmenter = Segmenter::new(SR);

        let utterances = feed(&mut segmenter, &speech(16.0));

        assert_eq!(utterances.len(), 1);
        assert!((15.0..15.1).contains(&secs(&utterances[0])));
    }

    #[test]
    fn flush_returns_the_utterance_in_progress() {
        let mut segmenter = Segmenter::new(SR);
        feed(&mut segmenter, &speech(1.0));

        let flushed = segmenter.flush();

        assert!(flushed.is_some_and(|samples| secs(&samples) >= 0.9));
        assert!(segmenter.flush().is_none());
    }
}
