import { useState } from 'react';
import { Captions } from 'lucide-react';
import { ExtensionPageLayout } from '@/features/extensions/extension-page-layout/extension-page-layout';
import { ExtensionHeader } from '@/features/extensions/extension-header/extension-header';
import { useTranslation } from '@/i18n';
import { SettingsUI } from '@/components/settings-ui';
import { Typography } from '@/components/typography';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/select';
import { Tabs, type TabItem } from '@/components/tabs';
import { CaptionExportButton } from './caption-export-button/caption-export-button';
import { CaptionList } from './caption-list/caption-list';
import { useLiveCaptions } from './hooks/use-live-captions';
import { NO_TRANSLATION } from './live-captions.helpers';
import type { CaptionLang } from './live-captions.types';

// Language names stay in their own language, whatever the interface language.
const LANGUAGE_TABS: TabItem<CaptionLang>[] = [
    { value: 'en', label: 'English' },
    { value: 'fr', label: 'Français' },
];

export const LiveCaptions = () => {
    const { t } = useTranslation();
    const liveCaptions = useLiveCaptions();
    const [lang, setLang] = useState<CaptionLang>('en');

    return (
        <ExtensionPageLayout colorScheme="standard" isActive={liveCaptions.isRunning}>
            <ExtensionHeader
                icon={Captions}
                title={t('Live Captions')}
                colorScheme="standard"
                titleTestId="live-captions-title"
                subtitle={t('Transcribes what your computer is playing, live, in English and in French.')}
                statusLabel={t('Listening')}
                toggle={{
                    checked: liveCaptions.isRunning,
                    onCheckedChange: (checked) => {
                        if (!liveCaptions.isBusy) {
                            void liveCaptions.setRunning(checked);
                        }
                    },
                    ariaLabel: t('Enable Live Captions'),
                    testId: 'live-captions-toggle',
                }}
            />
            <SettingsUI.Container>
                <SettingsUI.Item>
                    <SettingsUI.Description>
                        <Typography.Title>{t('Translation model')}</Typography.Title>
                        <Typography.Paragraph>
                            {t(
                                'Ollama model translating each sentence once it is transcribed. Transcription always comes first: the translation may lag behind.'
                            )}
                        </Typography.Paragraph>
                    </SettingsUI.Description>
                    <Select
                        value={liveCaptions.model}
                        onValueChange={liveCaptions.selectModel}
                        disabled={liveCaptions.isRunning}
                    >
                        <SelectTrigger className="w-[240px]" data-testid="live-captions-model">
                            <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                            <SelectItem value={NO_TRANSLATION}>{t('No translation')}</SelectItem>
                            {liveCaptions.models.map((name) => (
                                <SelectItem key={name} value={name}>
                                    {name}
                                </SelectItem>
                            ))}
                        </SelectContent>
                    </Select>
                </SettingsUI.Item>
            </SettingsUI.Container>
            <Tabs value={lang} onValueChange={setLang} tabs={LANGUAGE_TABS}>
                <div className="flex justify-end">
                    <CaptionExportButton lang={lang} disabled={liveCaptions.captions.length === 0} />
                </div>
                <CaptionList captions={liveCaptions.captions} lang={lang} />
            </Tabs>
        </ExtensionPageLayout>
    );
};
