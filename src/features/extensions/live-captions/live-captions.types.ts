export type CaptionLang = 'en' | 'fr';

export interface CaptionSegmentEvent {
    id: number;
    text: string;
    lang: CaptionLang;
    is_final: boolean;
}

export interface CaptionTranslationEvent {
    id: number;
    text: string;
}

export interface Caption {
    id: number;
    text: string;
    lang: CaptionLang;
    isFinal: boolean;
    translation?: string;
}

export interface CaptionLine {
    id: number;
    text: string;
    /** Still being transcribed, or shown in its original language until translated. */
    isPending: boolean;
}
