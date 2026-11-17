import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'react-toastify';
import { useTranslation } from '@/i18n';
import { applySegment, applyTranslation } from '@/features/extensions/live-captions/live-captions.helpers';
import { getCaptions, setCaptions } from '@/features/extensions/live-captions/live-captions.store';
import type {
    CaptionSegmentEvent,
    CaptionTranslationEvent,
} from '@/features/extensions/live-captions/live-captions.types';

export const LiveCaptionsListener = () => {
    const { t } = useTranslation();

    useEffect(() => {
        const unlistenPromises = [
            listen<CaptionSegmentEvent>('live-captions-segment', (event) => {
                setCaptions(applySegment(getCaptions(), event.payload));
            }),
            listen<CaptionTranslationEvent>('live-captions-translation', (event) => {
                setCaptions(applyTranslation(getCaptions(), event.payload));
            }),
            listen<string>('live-captions-error', (event) => {
                toast.error(t('Live Captions translation failed: {{error}}', { error: event.payload }), {
                    toastId: 'live-captions-error',
                    autoClose: 5000,
                });
            }),
        ];

        return () => {
            unlistenPromises.forEach((unlisten) => void unlisten.then((fn) => fn()));
        };
    }, [t]);

    return null;
};
