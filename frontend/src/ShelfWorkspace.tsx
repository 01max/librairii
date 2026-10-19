import {useCallback, useEffect, useRef, useState} from 'react';
import {OpenShelf, RenameShelf, DuplicateShelf, DeleteShelf, RemoveStoriesFromShelf, ReorderShelfStories} from '../wailsjs/go/main/App';
import {collections, library, shelves} from '../wailsjs/go/models';
import {type CollectionQuery, DEFAULT_COLLECTION_QUERY} from './query-codec';
import {t} from './i18n';
import ShelfReview from './ShelfReview';
import ShelfSources from './ShelfSources';
import ShelfDialog from './ShelfDialog';
import StoryCover from './StoryCover';
import OutcomeText from './OutcomeText';

export default function ShelfWorkspace({id, revision, query, onQuery, collections: sourceOptions, onChanged, onDeleted, onInspect}: {
    collections: collections.Summary[]; id: number; revision: number; query: CollectionQuery; onQuery: (query: CollectionQuery) => void;
    onChanged: () => void; onDeleted: () => void; onInspect: (story: library.StorySummary) => void;
}) {
    const [manualOrder, setManualOrder] = useState(true);
    const [view, setView] = useState<shelves.Evaluation | null>(null);
    const [selected, setSelected] = useState<string[]>([]);
    const [error, setError] = useState('');
    const [busy, setBusy] = useState(false);
    const [reviewOpen, setReviewOpen] = useState(false);
    const [manageSources, setManageSources] = useState(false);
    const [dialog, setDialog] = useState<'rename' | 'duplicate' | 'delete' | null>(null);
    const [name, setName] = useState('');
    const generation = useRef(0);
    const load = useCallback(async () => {
        const request = ++generation.current;
        try {
            const response = await OpenShelf(id, new library.StoryLibraryQuery({...query, sort: manualOrder ? '' : query.sort}));
            if (request !== generation.current) return;
            if (!response.evaluation) throw new Error(response.error?.message ?? t('Shelf could not be opened.'));
            setView(response.evaluation); setError('');
        } catch (error) { if (request === generation.current) setError(error instanceof Error ? error.message : t('Shelf could not be opened.')); }
    }, [id, query, manualOrder]);
    useEffect(() => { const pending = generation; const timer = window.setTimeout(() => void load(), 0); return () => {window.clearTimeout(timer); pending.current++;}; }, [load, revision]);
    const act = async (action: () => Promise<{error?: {message: string}}>) => {
        setBusy(true); setError('');
        try { const result = await action(); if (result.error) throw new Error(result.error.message); await load(); onChanged(); }
        catch (error) { setError(error instanceof Error ? error.message : t('Shelf could not be changed.')); }
        finally { setBusy(false); }
    };
    const reorder = async (uuid: string, delta: number) => {
        // Resolve the complete current manual order without rendering every member.
        const all: string[] = [];
        for (let page = 1; ; page++) {
            const response = await OpenShelf(id, new library.StoryLibraryQuery({...DEFAULT_COLLECTION_QUERY, sort: '', page, pageSize: 200}));
            if (!response.evaluation) return {error: response.error ?? {message: t('Shelf could not be opened.')}};
            all.push(...response.evaluation.members.map((member) => member.uuid));
            if (page >= response.evaluation.totalPages) break;
        }
        const index = all.indexOf(uuid), next = index + delta;
        if (index < 0 || next < 0 || next >= all.length) return {};
        [all[index], all[next]] = [all[next], all[index]];
        return ReorderShelfStories(id, all);
    };
    if (!view) return <section><h2>{t('Shelf')}</h2>{error && <p role="alert"><OutcomeText text={error}/></p>}<button type="button" onClick={() => void load()}>{t('Retry')}</button></section>;
    return <section className="curated-workspace" aria-label={t('Curated Shelf')}>
        <div className="heading"><div><h2>{view.shelf.name}</h2><p>{t('{0} approved stories · {1} unavailable', view.shelf.count, view.shelf.unavailableCount)}</p></div>
            <div className="curated-actions">{(['rename', 'duplicate', 'delete'] as const).map((action) => <button key={action} type="button" disabled={busy}
                onClick={() => {setName(action === 'rename' ? view.shelf.name : ''); setDialog(action);}}>{t(action === 'rename' ? 'Rename Shelf' : action === 'duplicate' ? 'Duplicate Shelf' : 'Delete Shelf')}</button>)}</div></div>
        <div className="curated-actions"><button type="button" disabled={busy} onClick={() => setReviewOpen(true)}>{t('Review updates')}</button><button type="button" disabled={busy} onClick={() => setManageSources(true)}>{t('Manage sources')}</button></div>
        <p>{t('Sources (any match): {0}', view.sources.filter((source) => source.active).map((source) => source.name).join(', ') || t('None'))}</p>
        {reviewOpen && <ShelfReview id={id} onClose={() => setReviewOpen(false)} onApplied={() => {void load(); onChanged();}}/>}
        {manageSources && <ShelfSources id={id} sources={view.sources} collections={sourceOptions} onClose={() => setManageSources(false)} onChanged={() => {void load(); onChanged();}}/>}
        <label>{t('Refine this view')}<input type="search" value={query.name} onChange={(event) => onQuery({...query, name: event.target.value, page: 1})}/></label>
        <label>{t('Sort stories')}<select value={manualOrder ? '' : query.sort} onChange={(event) => {setManualOrder(event.target.value === ''); if (event.target.value) onQuery({...query, sort: event.target.value as CollectionQuery['sort'], page: 1});}}>
            <option value="">{t('Manual order')}</option><option value="name_asc">{t('Name A–Z')}</option><option value="imported_desc">{t('Newest imported')}</option></select></label>
        <p>{t('Refinements change this view only. Approved membership stays the same.')}</p>
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
        <div className="curated-actions"><button type="button" disabled={busy || !selected.length}
            onClick={() => void act(async () => {const result = await RemoveStoriesFromShelf(id, selected); if (!result.error) setSelected([]); return result;})}>{t('Remove selected and exclude from suggestions')}</button></div>
        <ul className="curated-members">{view.members.map((member) => <li key={member.uuid} className={member.story?.broken ? 'is-broken' : ''}>
            <label><input type="checkbox" checked={selected.includes(member.uuid)} disabled={busy} onChange={() => setSelected(selected.includes(member.uuid) ? selected.filter((uuid) => uuid !== member.uuid) : [...selected, member.uuid])}/>{member.story?.title ?? member.cachedTitle}</label>
            {member.story ? <button type="button" className="curated-cover" onClick={() => onInspect(member.story!)} aria-label={t('Details for {0}', member.story.title)}><StoryCover story={member.story}/></button> : <p>{t('Unavailable')} · {member.uuid}</p>}
            {member.story?.broken && <p>{t('Broken · excluded from exports')}</p>}
            <small>{member.protected ? t('Protected') : t('Source-derived')} · {t('Added from: {0}', (member.origins ?? []).map((origin) => origin.name + (origin.active ? '' : ` (${t('detached')})`)).join(', ') || t('Manual selection'))}</small>
            {manualOrder && !query.name && <div className="curated-actions"><button type="button" disabled={busy || member.position === 0} aria-label={t('Move {0} up', member.cachedTitle)} onClick={() => void act(() => reorder(member.uuid, -1))}>↑</button>
                <button type="button" disabled={busy || member.position >= view.shelf.count - 1} aria-label={t('Move {0} down', member.cachedTitle)} onClick={() => void act(() => reorder(member.uuid, 1))}>↓</button></div>}
        </li>)}</ul>
        {!view.totalItems && <p>{t('No approved stories in this view.')}</p>}
        <nav className="pagination" aria-label={t('Shelf pages')}><button type="button" disabled={query.page <= 1} onClick={() => onQuery({...query, page: query.page - 1})}>{t('← Previous')}</button>
            <span>{t('Page {0} of {1}', view.page, Math.max(1, view.totalPages))}</span><button type="button" disabled={view.page >= view.totalPages} onClick={() => onQuery({...query, page: query.page + 1})}>{t('Next →')}</button></nav>
        {dialog && <ShelfDialog title={t(dialog === 'rename' ? 'Rename Shelf' : dialog === 'duplicate' ? 'Duplicate Shelf' : 'Delete Shelf')} busy={busy} onClose={() => setDialog(null)}>
            {dialog === 'delete' ? <p>{t('Delete this Shelf and its decisions? Library stories remain available.')}</p> : <label>{t('Shelf name')}<input value={name} onChange={(event) => setName(event.target.value)}/></label>}
            <button type="button" disabled={busy || (dialog !== 'delete' && !name.trim())} onClick={() => void act(async () => {
                const result = dialog === 'rename' ? await RenameShelf(id, name) : dialog === 'duplicate' ? await DuplicateShelf(id, name) : await DeleteShelf(id);
                if (!result.error) {setDialog(null); if (dialog === 'delete') onDeleted();} return result;
            })}>{t('Confirm')}</button>
        </ShelfDialog>}
    </section>;
}
