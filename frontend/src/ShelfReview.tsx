import {useCallback, useEffect, useState} from 'react';
import {PreviewShelfUpdate, ApplyShelfUpdate} from '../wailsjs/go/main/App';
import {shelves} from '../wailsjs/go/models';
import ShelfDialog from './ShelfDialog';
import StoryCover from './StoryCover';
import OutcomeText from './OutcomeText';
import {t} from './i18n';

export default function ShelfReview({id, onClose, onApplied}: {id: number; onClose: () => void; onApplied: () => void}) {
    const [review, setReview] = useState<shelves.Review | null>(null);
    const [choices, setChoices] = useState<Record<string, string>>({});
    const [page, setPage] = useState(0), [busy, setBusy] = useState(false), [error, setError] = useState('');
    const [refreshRequired, setRefreshRequired] = useState(false);
    const preview = useCallback(async () => {
        setBusy(true); setError('');
        try {
            const response = await PreviewShelfUpdate(id);
            if (!response.review) throw new Error(response.error?.message ?? t('Review could not be loaded.'));
            setReview(response.review); setChoices({}); setPage(0); setRefreshRequired(false);
        } catch (error) {setError(error instanceof Error ? error.message : t('Review could not be loaded.'));}
        finally {setBusy(false);}
    }, [id]);
    useEffect(() => {const timer = window.setTimeout(() => void preview(), 0); return () => window.clearTimeout(timer);}, [preview]);
    const entries = [...(review?.additions ?? []).map((candidate) => ({candidate, kind: 'addition'})), ...(review?.removals ?? []).map((candidate) => ({candidate, kind: 'removal'}))];
    const approved = Object.entries(choices).filter(([, action]) => action && action !== 'defer');
    const apply = async () => {
        if (!review) return; setBusy(true); setError('');
        try {
            const response = await ApplyShelfUpdate(id, review.token, approved.map(([uuid, action]) => new shelves.Decision({uuid, action})));
            if (response.error) {setRefreshRequired(true); throw new Error(response.error.message);}
            onApplied(); onClose();
        } catch (error) {setError(error instanceof Error ? error.message : t('Review could not be applied.'));}
        finally {setBusy(false);}
    };
    const bulk = (kind: string, action: string) => setChoices((previous) => ({...previous, ...Object.fromEntries(entries.filter((entry) => entry.kind === kind).map(({candidate}) => [candidate.uuid, action]))}));
    return <ShelfDialog title={t('Review Shelf updates')} busy={busy} onClose={onClose}>
        <p>{t('Suggestions match any linked Collection. Only approved decisions change this Shelf.')}</p>
        {review?.incomplete && <p role="status">{t('Review incomplete. Healthy sources can suggest additions; removals are suspended.')}</p>}
        {review?.upToDate && <p role="status">{t('Up to date')}</p>}
        {(review?.warnings ?? []).map((warning, index) => <p key={index}><OutcomeText text={warning.message}/></p>)}
        {review && <>
            <p>{t('{0} additions · {1} removals', review.additions.length, review.removals.length)}</p>
            <div className="curated-actions"><button type="button" disabled={busy || !review.additions.length} onClick={() => bulk('addition', 'add')}>{t('Approve all additions')}</button>
                <button type="button" disabled={busy || !review.additions.length} onClick={() => bulk('addition', 'exclude')}>{t('Exclude all additions')}</button>
                <button type="button" disabled={busy || !review.removals.length} onClick={() => bulk('removal', 'keep')}>{t('Keep all removals')}</button>
                <button type="button" disabled={busy || !review.removals.length} onClick={() => bulk('removal', 'remove')}>{t('Approve all removals')}</button>
                <button type="button" disabled={busy} onClick={() => setChoices({})}>{t('Defer all')}</button></div>
            <ul className="curated-candidates">{entries.slice(page * 25, (page + 1) * 25).map(({candidate, kind}) => <li key={candidate.uuid}>
                <strong>{kind === 'addition' ? t('Suggested addition') : t('Suggested removal')}</strong>
                <label>{candidate.title}<select disabled={busy} aria-label={t('Decision for {0}', candidate.title)} value={choices[candidate.uuid] ?? 'defer'} onChange={(event) => setChoices({...choices, [candidate.uuid]: event.target.value})}>
                    <option value="defer">{t('Not now')}</option>
                    {kind === 'addition' ? <><option value="add">{t('Add')}</option><option value="exclude">{t('Exclude from suggestions')}</option></> : <><option value="keep">{t('Keep and protect')}</option><option value="remove">{t('Remove and exclude')}</option></>}
                </select></label>
                {candidate.story && <div className="curated-preview"><StoryCover story={candidate.story}/></div>}
                {candidate.story?.broken && <p>{t('Broken · excluded from exports')}</p>}
                <p>{kind === 'addition' ? t('Currently matches: {0}', (candidate.sources ?? []).map((source) => source.name).join(', ')) : t('No longer matches any linked Collection.')}</p>
                {!!candidate.origins?.length && <small>{t('Added from: {0}', candidate.origins.map((source) => source.name + (source.active ? '' : ` (${t('detached')})`)).join(', '))}</small>}
            </li>)}</ul>
            <nav className="pagination" aria-label={t('Review pages')}><button type="button" disabled={page === 0} onClick={() => setPage(page - 1)}>{t('← Previous')}</button>
                <span>{t('Page {0} of {1}', page + 1, Math.max(1, Math.ceil(entries.length / 25)))}</span><button type="button" disabled={(page + 1) * 25 >= entries.length} onClick={() => setPage(page + 1)}>{t('Next →')}</button></nav>
            <p aria-live="polite">{t('Approved: {0} add · {1} exclude · {2} keep · {3} remove', ...['add', 'exclude', 'keep', 'remove'].map((action) => approved.filter(([, value]) => value === action).length))}</p>
            <button type="button" disabled={busy || !approved.length || refreshRequired} onClick={() => void apply()}>{t('Apply {0} approved changes', approved.length)}</button>
        </>}
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
        {refreshRequired && <p>{t('Refresh the preview and review your decisions again.')}</p>}
        <button type="button" disabled={busy} onClick={() => void preview()}>{t('Refresh preview')}</button>
    </ShelfDialog>;
}
