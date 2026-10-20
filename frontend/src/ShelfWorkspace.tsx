import TagSearchFilters from './TagSearchFilters';
import SearchInput from './SearchInput';
import {useCallback, useEffect, useRef, useState} from 'react';
import {OpenShelf, RenameShelf, DuplicateShelf, DeleteShelf, RemoveStoriesFromShelf, ReorderShelfStories, SetShelfStoryProtected, ClearShelfExclusion} from '../wailsjs/go/main/App';
import {collections, exporter, library, shelves, tagging} from '../wailsjs/go/models';
import {type CollectionQuery, DEFAULT_COLLECTION_QUERY} from './query-codec';
import {t} from './i18n';
import {type LibraryView} from './library-view';
import {useStoryFlags} from './use-story-flags';
import BrokenToggle from './BrokenToggle';
import ShelfReview from './ShelfReview';
import ShelfSources from './ShelfSources';
import ShelfDialog from './ShelfDialog';
import StoryCover from './StoryCover';
import OutcomeText from './OutcomeText';

export default function ShelfWorkspace({catalog, id, revision, query, onQuery, collections: sourceOptions, onChanged, onDeleted, onInspect, onExport, manualOrder = true, onManualOrder, viewMode = 'large', exportBusy = false}: {
    catalog?: tagging.Catalog | null; exportBusy?: boolean; viewMode?: LibraryView; manualOrder?: boolean; onManualOrder?: (manual: boolean) => void; collections: collections.Summary[]; id: number; revision: number; query: CollectionQuery; onQuery: (query: CollectionQuery) => void;
    onExport: (request: exporter.PreflightRequest) => void; onChanged: () => void; onDeleted: () => void; onInspect: (story: library.StorySummary) => void;
}) {
    const [view, setView] = useState<shelves.Evaluation | null>(null);
    const [exclusionsOpen, setExclusionsOpen] = useState(false);
    const [exclusionPage, setExclusionPage] = useState(0);
    const [knownMembers, setKnownMembers] = useState<Record<string, shelves.Member>>({});
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
            setView(response.evaluation); setKnownMembers((previous) => ({...previous, ...Object.fromEntries(response.evaluation!.members.map((member) => [member.uuid, member]))})); setError('');
        } catch (error) { if (request === generation.current) setError(error instanceof Error ? error.message : t('Shelf could not be opened.')); }
    }, [id, query, manualOrder]);
    useEffect(() => { const pending = generation; const timer = window.setTimeout(() => void load(), 0); return () => {window.clearTimeout(timer); pending.current++;}; }, [load, revision]);
    const act = async (action: () => Promise<{error?: {message: string}}>) => {
        setBusy(true); setError('');
        try { const result = await action(); if (result.error) throw new Error(result.error.message); await load(); onChanged(); }
        catch (error) { setError(error instanceof Error ? error.message : t('Shelf could not be changed.')); }
        finally { setBusy(false); }
    };
    const onMembershipChanged = (removed: string[]) => {
        setSelected((previous) => previous.filter((uuid) => !removed.includes(uuid)));
        setKnownMembers((previous) => Object.fromEntries(Object.entries(previous).filter(([uuid]) => !removed.includes(uuid))));
        void load(); onChanged();
    };
    const onFlagsChanged = useCallback(async () => {await load(); onChanged();}, [load, onChanged]);
    const flags = useStoryFlags(onFlagsChanged);
    const selectedStoryIDs = selected.flatMap((uuid) => knownMembers[uuid]?.story ? [knownMembers[uuid].story!.id] : []);
    const reorder = async (uuid: string, delta: number) => {
        // Resolve the complete current manual order without rendering every member.
        const all: string[] = [];
        for (let page = 1; ; page++) {
            const response = await OpenShelf(id, new library.StoryLibraryQuery({...DEFAULT_COLLECTION_QUERY, sort: '', page, pageSize: 100}));
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
        {reviewOpen && <ShelfReview id={id} onClose={() => setReviewOpen(false)} onApplied={onMembershipChanged}/>}
        {manageSources && <ShelfSources id={id} sources={view.sources} collections={sourceOptions} onClose={() => setManageSources(false)} onChanged={onMembershipChanged}/>}
        <div className="curated-actions">
            <button type="button" disabled={exportBusy} onClick={() => onExport(new exporter.PreflightRequest({sourceType: 'curated_shelf', shelfIds: [id], query: {}}))}>{t('Export whole Shelf')}</button>
            <button type="button" disabled={exportBusy} onClick={() => onExport(new exporter.PreflightRequest({sourceType: 'current_query', queryShelfId: id, query}))}>{t('Export current result')}</button>
            <button type="button" disabled={exportBusy || !selected.length} onClick={() => onExport(new exporter.PreflightRequest({sourceType: 'selection', queryShelfId: id, shelfSelectionUuids: selected, query: {}}))}>{t('Export selection')}</button>
            <button type="button" onClick={() => setExclusionsOpen(true)}>{t('Manage exclusions ({0})', view.exclusions.length)}</button>
        </div>
        {flags.error && <p role="alert"><OutcomeText text={flags.error}/></p>}
        {flags.notice && <p role="status">{flags.notice} {flags.canUndo && <button type="button" disabled={flags.pending} onClick={() => void flags.undo()}>{t('Undo')}</button>}</p>}
        <BrokenToggle title={t('Selected stories')} disabled={flags.pending || !selectedStoryIDs.length}
            broken={selectedStoryIDs.length > 0 && selectedStoryIDs.every((id) => Object.values(knownMembers).some((member) => member.story?.id === id && member.story.broken))}
            onChange={(broken) => void flags.set(selectedStoryIDs, broken)}/>
        <div className="shelf-search"><label>{t('Refine this view')}<SearchInput value={query.name} onChange={(name) => onQuery({...query, name, page: 1})}/></label><TagSearchFilters catalog={catalog} query={query} onChange={onQuery}/></div>
        <label>{t('Sort stories')}<select value={manualOrder ? '' : query.sort} onChange={(event) => {onManualOrder?.(event.target.value === ''); if (event.target.value) onQuery({...query, sort: event.target.value as CollectionQuery['sort'], page: 1});}}>
            <option value="">{t('Manual order')}</option><option value="name_asc">{t('Name A–Z')}</option><option value="imported_desc">{t('Newest imported')}</option></select></label>
        <p>{t('Refinements change this view only. Approved membership stays the same.')}</p>
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
        <div className="curated-actions"><button type="button" disabled={busy || !selected.length}
            onClick={() => void act(async () => {const result = await RemoveStoriesFromShelf(id, selected); if (!result.error) setSelected([]); return result;})}>{t('Remove selected and exclude from suggestions')}</button></div>
        <ul className={`curated-members curated-${viewMode}`}>{view.members.map((member) => <li key={member.uuid} className={member.story?.broken ? 'is-broken' : ''}>
            <div className="curated-member-info">
                <label><input type="checkbox" checked={selected.includes(member.uuid)} disabled={busy} onChange={() => setSelected(selected.includes(member.uuid) ? selected.filter((uuid) => uuid !== member.uuid) : [...selected, member.uuid])}/><span>{member.story?.title ?? member.cachedTitle}</span></label>
                <small>{member.protected ? t('Protected') : t('Source-derived')} · {t('Added from: {0}', (member.origins ?? []).map((origin) => origin.name + (origin.active ? '' : ` (${t('detached')})`)).join(', ') || t('Manual selection'))}
                    {member.story?.broken && <> · <span className="curated-broken">{t('Broken · excluded from exports')}</span></>}</small>
            </div>
            {member.story ? <button type="button" className="curated-cover" onClick={() => onInspect(member.story!)} aria-label={t('Details for {0}', member.story.title)}>{viewMode === 'list' ? t('Open details') : <StoryCover story={member.story}/>}</button> : <p className="curated-unavailable" title={member.uuid}>{t('Unavailable')}<span className="curated-unavailable-id"> · {member.uuid}</span></p>}
            <button className="curated-protection" type="button" disabled={busy || (!view.sources.some((source) => source.active) && member.protected)} onClick={() => void act(() => SetShelfStoryProtected(id, member.uuid, !member.protected))}>{member.protected ? t('Unprotect {0}', member.cachedTitle) : t('Protect {0}', member.cachedTitle)}</button>
            {manualOrder && !query.name && <div className="curated-actions curated-member-order"><button type="button" disabled={busy || member.position === 0} aria-label={t('Move {0} up', member.cachedTitle)} onClick={() => void act(() => reorder(member.uuid, -1))}>↑</button>
                <button type="button" disabled={busy || member.position >= view.shelf.count - 1} aria-label={t('Move {0} down', member.cachedTitle)} onClick={() => void act(() => reorder(member.uuid, 1))}>↓</button></div>}
        </li>)}</ul>
        {!view.totalItems && <p>{t('No approved stories in this view.')}</p>}
        <nav className="pagination" aria-label={t('Shelf pages')}><button type="button" disabled={view.page <= 1} onClick={() => onQuery({...query, page: view.page - 1})}>{t('← Previous')}</button>
            <span>{t('Page {0} of {1}', view.page, Math.max(1, view.totalPages))}</span><button type="button" disabled={view.page >= view.totalPages} onClick={() => onQuery({...query, page: view.page + 1})}>{t('Next →')}</button></nav>
        {exclusionsOpen && <ShelfDialog title={t('Manage exclusions')} busy={busy} onClose={() => setExclusionsOpen(false)}>
            <p>{t('Clearing an exclusion makes the story eligible for suggestions; it does not add membership.')}</p>
            <ul className="curated-candidates">{view.exclusions.slice(exclusionPage * 25, (exclusionPage + 1) * 25).map((item) => <li key={item.uuid}>{item.title || item.uuid}
                <button type="button" disabled={busy} onClick={() => void act(() => ClearShelfExclusion(id, item.uuid))}>{t('Allow suggestions for {0}', item.title || item.uuid)}</button></li>)}</ul>
            <div className="curated-actions"><button type="button" disabled={exclusionPage === 0} onClick={() => setExclusionPage(exclusionPage - 1)}>{t('← Previous')}</button><button type="button" disabled={(exclusionPage + 1) * 25 >= view.exclusions.length} onClick={() => setExclusionPage(exclusionPage + 1)}>{t('Next →')}</button></div>
        </ShelfDialog>}
        {dialog && <ShelfDialog title={t(dialog === 'rename' ? 'Rename Shelf' : dialog === 'duplicate' ? 'Duplicate Shelf' : 'Delete Shelf')} busy={busy} onClose={() => setDialog(null)}>
            {dialog === 'delete' ? <p>{t('Delete this Shelf and its decisions? Library stories remain available.')}</p> : <label>{t('Shelf name')}<input value={name} onChange={(event) => setName(event.target.value)}/></label>}
            <button type="button" disabled={busy || (dialog !== 'delete' && !name.trim())} onClick={() => void act(async () => {
                const result = dialog === 'rename' ? await RenameShelf(id, name) : dialog === 'duplicate' ? await DuplicateShelf(id, name) : await DeleteShelf(id);
                if (!result.error) {setDialog(null); if (dialog === 'delete') onDeleted();} return result;
            })}>{t('Confirm')}</button>
        </ShelfDialog>}
    </section>;
}
