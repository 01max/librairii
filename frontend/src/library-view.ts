import {useEffect, useState} from 'react';

export type LibraryView = 'list' | 'small' | 'large';

const storageKey = 'librairii.library-view';

export function useLibraryView() {
    const [view, setView] = useState<LibraryView>(() => {
        try {
            const saved = window.localStorage.getItem(storageKey);
            if (saved === 'list' || saved === 'small') return saved;
        } catch {
            // Browsing still works when preference storage is unavailable.
        }
        return 'large';
    });

    useEffect(() => {
        try {
            window.localStorage.setItem(storageKey, view);
        } catch {
            // Keep the current session's choice even without storage access.
        }
    }, [view]);

    return [view, setView] as const;
}
