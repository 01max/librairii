import {useCallback, useRef, useState} from 'react';
import {RestoreStoryFlags, SetStoriesBroken} from '../wailsjs/go/main/App';
import {library} from '../wailsjs/go/models';
import {t} from './i18n';
import type {BrokenVisibility} from './query-codec';

type FlagResponse = {result?: library.FlagResult; error?: {message: string}};

export function useStoryFlags(
    onChanged: (result: library.FlagResult) => Promise<void>,
    visibility: BrokenVisibility = 'hide',
) {
    const locked = useRef(false);
    const [pending, setPending] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [notice, setNotice] = useState<string | null>(null);
    const [undoChanges, setUndoChanges] = useState<library.FlagChange[]>([]);

    const execute = useCallback(async (request: () => Promise<FlagResponse>, broken?: boolean) => {
        if (locked.current) return;
        locked.current = true;
        setPending(true);
        setError(null);
        try {
            const response = await request();
            if (!response.result) {
                setError(response.error?.message ?? t('Story flags could not be updated. Try again.'));
                return;
            }
            const result = response.result;
            setUndoChanges(broken === undefined ? [] : result.changes);
            if (!result.changedStories) setNotice(t('No broken flags changed.'));
            else if (broken === undefined) setNotice(t('Previous broken flags restored.'));
            else {
                const counts = t('{0} of {1} stories updated.', result.changedStories, result.requestedStories);
                setNotice(broken && visibility === 'hide'
                    ? t('{0} Marked as broken and hidden.', counts)
                    : !broken && visibility === 'only'
                        ? t('{0} Cleared flags; stories no longer match Broken only.', counts)
                        : counts);
            }
            await onChanged(result);
        } catch {
            setError(t('Story flags could not be updated. Try again.'));
        } finally {
            locked.current = false;
            setPending(false);
        }
    }, [onChanged, visibility]);

    const set = useCallback((ids: number[], broken: boolean) =>
        execute(() => SetStoriesBroken(ids, broken), broken), [execute]);
    const undo = useCallback(() => undoChanges.length
        ? execute(() => RestoreStoryFlags(undoChanges)) : Promise.resolve(), [execute, undoChanges]);

    return {pending, error, notice, canUndo: undoChanges.length > 0, set, undo};
}
