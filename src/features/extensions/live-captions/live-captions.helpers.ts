import type {
    Caption,
    CaptionLang,
    CaptionLine,
    CaptionSegmentEvent,
    CaptionTranslationEvent,
} from './live-captions.types';

export const NO_TRANSLATION = 'none';
export const MODEL_STORAGE_KEY = 'live-captions-model';
const MAX_CAPTIONS = 500;

// Ranked on a 20-sentence English/French benchmark (CPU only, 4 threads): fidelity first, then latency.
// Reasoning models are left out: they think for a minute before translating one sentence.
const PREFERRED_MODEL_PREFIXES = ['gemma4:e2b', 'gemma3:4b', 'translategemma:4b', 'gemma3:1b'];

/** Model used until the user picks one: the best-ranked installed model, else Ollama's first. */
export const pickDefaultModel = (models: string[]): string => {
    for (const prefix of PREFERRED_MODEL_PREFIXES) {
        const match = models.find((name) => name.startsWith(prefix));
        if (match != null) {
            return match;
        }
    }
    return models[0] ?? NO_TRANSLATION;
};

/** A partial and its final share an id: the newest replaces the previous one. An empty final discards it. */
export const applySegment = (captions: Caption[], event: CaptionSegmentEvent): Caption[] => {
    const others = captions.filter((caption) => caption.id !== event.id);
    if (event.text.length === 0) {
        return others;
    }
    const caption: Caption = { id: event.id, text: event.text, lang: event.lang, isFinal: event.is_final };
    return [...others, caption].slice(-MAX_CAPTIONS);
};

export const applyTranslation = (captions: Caption[], event: CaptionTranslationEvent): Caption[] =>
    captions.map((caption) => (caption.id === event.id ? { ...caption, translation: event.text } : caption));

/** What a tab shows for a caption: its text when spoken in that language, else its translation once ready. */
export const getCaptionLine = (caption: Caption, lang: CaptionLang): CaptionLine => {
    if (caption.lang === lang) {
        return { id: caption.id, text: caption.text, isPending: !caption.isFinal };
    }
    if (caption.translation != null) {
        return { id: caption.id, text: caption.translation, isPending: false };
    }
    return { id: caption.id, text: caption.text, isPending: true };
};

/** Exported file for a tab: one line per caption, exactly what the tab shows. */
export const formatTranscript = (captions: Caption[], lang: CaptionLang): string =>
    captions.map((caption) => `${getCaptionLine(caption, lang).text}\n`).join('');
