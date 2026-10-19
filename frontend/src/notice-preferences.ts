import {useEffect, useState} from 'react';

const storageKey = 'librairii.notice-preferences';

type NoticePreferences = {
    dismissedOperations: string[];
    luniiSyncDismissed: boolean;
};

function readPreferences(): NoticePreferences {
    try {
        const saved: unknown = JSON.parse(window.localStorage.getItem(storageKey) ?? 'null');
        if (saved && typeof saved === 'object' && 'dismissedOperations' in saved &&
            Array.isArray(saved.dismissedOperations) && 'luniiSyncDismissed' in saved) {
            return {
                dismissedOperations: saved.dismissedOperations.filter(
                    (id): id is string => typeof id === 'string',
                ),
                luniiSyncDismissed: saved.luniiSyncDismissed === true,
            };
        }
    } catch {
        // Notices still work for this session if preference storage is unavailable.
    }
    return {dismissedOperations: [], luniiSyncDismissed: false};
}

export function useNoticePreferences() {
    const [preferences, setPreferences] = useState(readPreferences);
    useEffect(() => {
        try {
            window.localStorage.setItem(storageKey, JSON.stringify(preferences));
        } catch {
            // Retain session behavior even if writing preferences fails.
        }
    }, [preferences]);

    return {
        dismissedOperations: new Set(preferences.dismissedOperations),
        luniiSyncDismissed: preferences.luniiSyncDismissed,
        dismissOperation: (id: string) => setPreferences((current) => ({
            ...current,
            dismissedOperations: [...new Set([...current.dismissedOperations, id])],
        })),
        setLuniiSyncDismissed: (dismissed: boolean) => setPreferences((current) => ({
            ...current, luniiSyncDismissed: dismissed,
        })),
    };
}
