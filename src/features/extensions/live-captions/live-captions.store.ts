import type { Caption } from './live-captions.types';

// Captions live outside React so they keep filling while another page is shown.
// Memory only: nothing is ever written to disk.
let captions: Caption[] = [];
const subscribers = new Set<() => void>();

export const getCaptions = () => captions;

export const setCaptions = (next: Caption[]) => {
    captions = next;
    subscribers.forEach((notify) => notify());
};

export const subscribeToCaptions = (notify: () => void) => {
    subscribers.add(notify);
    return () => {
        subscribers.delete(notify);
    };
};
