import { Upload } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { toast } from 'react-toastify';
import { Button } from '@/components/button';
import { useTranslation } from '@/i18n';
import { formatTranscript } from '../live-captions.helpers';
import { getCaptions } from '../live-captions.store';
import type { CaptionLang } from '../live-captions.types';

interface CaptionExportButtonProps {
    lang: CaptionLang;
    disabled: boolean;
}

export const CaptionExportButton = ({ lang, disabled }: CaptionExportButtonProps) => {
    const { t } = useTranslation();

    const handleExport = async () => {
        try {
            const filePath = await save({
                title: t('Select file to export captions'),
                filters: [{ name: t('Text file (.txt)'), extensions: ['txt', 'TXT'] }],
                defaultPath: `murmure-captions-${lang}.txt`,
            });
            if (filePath == null) {
                return;
            }
            // Read the store once the file is chosen: captions keep coming while the dialog is open.
            await invoke('export_live_captions', { filePath, content: formatTranscript(getCaptions(), lang) });
            toast.success(t('Captions exported successfully'), { autoClose: 2000 });
        } catch (error) {
            toast.error(t('Failed to export captions') + ' : ' + error);
        }
    };

    return (
        <Button
            variant="subtle"
            size="sm"
            disabled={disabled}
            onClick={handleExport}
            data-testid="live-captions-export"
        >
            <Upload />
            {t('Export')}
        </Button>
    );
};
