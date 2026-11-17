import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { toast } from 'react-toastify';
import { MODEL_STORAGE_KEY, NO_TRANSLATION, pickDefaultModel } from '../live-captions.helpers';
import { getCaptions, setCaptions, subscribeToCaptions } from '../live-captions.store';

interface OllamaModel {
    name: string;
}

export const useLiveCaptions = () => {
    const captions = useSyncExternalStore(subscribeToCaptions, getCaptions);
    const [isRunning, setIsRunning] = useState(false);
    const [isBusy, setIsBusy] = useState(false);
    const [models, setModels] = useState<string[]>([]);
    const [model, setModel] = useState(() => window.localStorage.getItem(MODEL_STORAGE_KEY) ?? NO_TRANSLATION);

    useEffect(() => {
        const load = async () => {
            try {
                setIsRunning(await invoke<boolean>('is_live_captions_running'));
                const { url } = await invoke<{ url: string }>('get_llm_connect_settings');
                const fetched = await invoke<OllamaModel[]>('fetch_ollama_models', { url });
                const names = fetched.map((entry) => entry.name);
                setModels(names);
                // Translate out of the box: without a saved, still installed choice, take the best-ranked model.
                const saved = window.localStorage.getItem(MODEL_STORAGE_KEY);
                const isSavedUsable = saved === NO_TRANSLATION || (saved != null && names.includes(saved));
                if (!isSavedUsable) {
                    setModel(pickDefaultModel(names));
                }
            } catch (error) {
                // Ollama is optional: without it the captions are simply not translated.
                console.error('Failed to load live captions state:', error);
            }
        };
        void load();
    }, []);

    const selectModel = (value: string) => {
        setModel(value);
        window.localStorage.setItem(MODEL_STORAGE_KEY, value);
    };

    const setRunning = async (shouldRun: boolean) => {
        setIsBusy(true);
        try {
            if (shouldRun) {
                setCaptions([]);
                await invoke('start_live_captions', { model: model === NO_TRANSLATION ? null : model });
            } else {
                await invoke('stop_live_captions');
            }
            setIsRunning(shouldRun);
        } catch (error) {
            toast.error(String(error));
        }
        setIsBusy(false);
    };

    return { captions, isRunning, isBusy, models, model, selectModel, setRunning };
};
