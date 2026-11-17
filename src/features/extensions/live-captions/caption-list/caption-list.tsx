import { useEffect, useRef } from 'react';
import clsx from 'clsx';
import { useTranslation } from '@/i18n';
import { Typography } from '@/components/typography';
import { getCaptionLine } from '../live-captions.helpers';
import type { Caption, CaptionLang } from '../live-captions.types';

interface CaptionListProps {
    captions: Caption[];
    lang: CaptionLang;
}

export const CaptionList = ({ captions, lang }: CaptionListProps) => {
    const { t } = useTranslation();
    const containerRef = useRef<HTMLDivElement>(null);

    // Follow the speech: keep the newest caption in view.
    useEffect(() => {
        const container = containerRef.current;
        if (container != null) {
            container.scrollTop = container.scrollHeight;
        }
    }, [captions, lang]);

    return (
        <div
            ref={containerRef}
            className="h-[calc(100vh-28rem)] min-h-40 space-y-2 overflow-y-auto text-base leading-relaxed"
            data-testid={`live-captions-${lang}`}
        >
            {captions.length === 0 && (
                <Typography.Paragraph>
                    {t('Captions appear here as soon as your computer plays speech.')}
                </Typography.Paragraph>
            )}
            {captions.map((caption) => {
                const line = getCaptionLine(caption, lang);
                return (
                    <p key={line.id} className={clsx(line.isPending && 'italic text-muted-foreground')}>
                        {line.text}
                    </p>
                );
            })}
        </div>
    );
};
