import {useCallback, useEffect, useState} from 'react';
import {PreviewShelfSources, ApplyShelfSources, PreviewCollectionDeletion, ApplyCollectionDeletion} from '../wailsjs/go/main/App';
import {collections, shelves} from '../wailsjs/go/models';
import ShelfDialog from './ShelfDialog';
import ReviewWarnings from './ReviewWarnings';
import OutcomeText from './OutcomeText';
import {t} from './i18n';

type Choices = Record<string, string>;
export function AffectedMembers({plan, choices, onChange, busy}: {plan: shelves.SourceChangeReview; choices: Choices; onChange: (choices: Choices) => void; busy: boolean}) {
    const [page, setPage] = useState(0);
    return <section aria-label={plan.shelfName}>
        <h4>{plan.shelfName} · {t('{0} affected stories', plan.affected.length)}</h4>
        {plan.incomplete && <p role="status">{t('Review incomplete. Removal is unavailable while source coverage is uncertain.')}</p>}
        <ReviewWarnings warnings={plan.warnings ?? []}/>
        <p>{t('Keep leaves the story on this Shelf. Remove also excludes future suggestions. Every affected story needs a decision.')}</p>
        <ul className="curated-candidates">{plan.affected.slice(page * 25, (page + 1) * 25).map((item) => <li key={item.uuid}>
            <label>{item.title}<select disabled={busy} value={choices[item.uuid] ?? ''} onChange={(event) => onChange({...choices, [item.uuid]: event.target.value})}>
                <option value="">{t('Not now')}</option><option value="keep">{t('Keep')}</option>
                <option value="remove" disabled={plan.incomplete || !item.story}>{t('Remove and exclude')}</option></select></label>
            <small>{t('Added from: {0}', (item.origins ?? []).map((origin) => origin.name).join(', ') || t('Manual selection'))}</small>
            {!item.story && <span>{t('Unavailable')}</span>}
        </li>)}</ul>
        {plan.affected.length > 25 && <div className="curated-actions"><button type="button" disabled={page === 0} onClick={() => setPage(page - 1)}>{t('← Previous')}</button>
            <span>{t('Page {0} of {1}', page + 1, Math.ceil(plan.affected.length / 25))}</span><button type="button" disabled={(page + 1) * 25 >= plan.affected.length} onClick={() => setPage(page + 1)}>{t('Next →')}</button></div>}
    </section>;
}
const decisions = (choices: Choices) => Object.entries(choices).filter(([, action]) => action).map(([uuid, action]) => new shelves.Decision({uuid, action}));
const complete = (plan: shelves.SourceChangeReview, choices: Choices) => plan.affected.every((item) => ['keep', 'remove'].includes(choices[item.uuid]));

export default function ShelfSources({id, sources, collections: options, onClose, onChanged}: {
    id: number; sources: shelves.Source[]; collections: collections.Summary[]; onClose: () => void; onChanged: (removed: string[]) => void;
}) {
    const [ids, setIDs] = useState(sources.filter((source) => source.active).map((source) => source.collectionId));
    const [plan, setPlan] = useState<shelves.SourceChangeReview | null>(null);
    const [choices, setChoices] = useState<Choices>({});
    const [busy, setBusy] = useState(false), [error, setError] = useState('');
    const preview = async () => {
        setBusy(true); setError('');
        try { const response = await PreviewShelfSources(id, ids); if (!response.review) throw new Error(response.error?.message ?? t('Source preview failed.')); setPlan(response.review); setChoices({}); }
        catch (error) { setError(error instanceof Error ? error.message : t('Source preview failed.')); } finally {setBusy(false);}
    };
    const apply = async (keepAll: boolean) => {
        if (!plan) return; setBusy(true); setError('');
        try {const response = await ApplyShelfSources(id, plan.token, keepAll, keepAll ? [] : decisions(choices)); if (response.error) throw new Error(response.error.message); onChanged(keepAll ? [] : Object.keys(choices).filter((uuid) => choices[uuid] === 'remove')); onClose();}
        catch (error) {setError(error instanceof Error ? error.message : t('Sources could not be changed.'));} finally {setBusy(false);}
    };
    return <ShelfDialog title={t('Manage sources')} busy={busy} onClose={onClose}>
        {!plan ? <><fieldset disabled={busy}><legend>{t('Suggest stories matching any of these Collections')}</legend>
            {options.map((source) => <label key={source.id}><input type="checkbox" checked={ids.includes(source.id)} onChange={() => setIDs(ids.includes(source.id) ? ids.filter((id) => id !== source.id) : [...ids, source.id])}/>{source.name}</label>)}
        </fieldset><p>{t('Linking sources never adds stories automatically.')}</p><button type="button" disabled={busy} onClick={() => void preview()}>{t('Preview source changes')}</button></> : <>
            <p>{t('Detached: {0}', plan.detached.map((source) => source.name).join(', ') || t('None'))}</p>
            <AffectedMembers key={plan.token} plan={plan} choices={choices} onChange={setChoices} busy={busy}/>
            <button type="button" disabled={busy} onClick={() => setPlan(null)}>{t('Change inputs')}</button>
            <button type="button" disabled={busy} onClick={() => void preview()}>{t('Refresh preview')}</button>
            <button type="button" disabled={busy} onClick={() => void apply(true)}>{t('Keep all and apply sources')}</button>
            <button type="button" disabled={busy || !complete(plan, choices)} onClick={() => void apply(false)}>{t('Apply reviewed source changes')}</button>
        </>}
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
    </ShelfDialog>;
}

export function CollectionDeletion({id, name, onClose, onDeleted}: {id: number; name: string; onClose: () => void; onDeleted: () => void}) {
    const [shelfPage, setShelfPage] = useState(0);
    const [plan, setPlan] = useState<shelves.CollectionDeletionReview | null>(null);
    const [choices, setChoices] = useState<Record<number, Choices>>({});
    const [busy, setBusy] = useState(false), [error, setError] = useState('');
    const preview = useCallback(async () => {
        setBusy(true); setError('');
        try {const response = await PreviewCollectionDeletion(id); if (!response.review) throw new Error(response.error?.message ?? t('Source preview failed.')); setPlan(response.review); setChoices({}); setShelfPage(0);}
        catch (error) {setError(error instanceof Error ? error.message : t('Source preview failed.'));} finally {setBusy(false);}
    }, [id]);
    useEffect(() => {const timer = window.setTimeout(() => void preview(), 0); return () => window.clearTimeout(timer);}, [preview]);
    const apply = async (keepAll: boolean) => {
        if (!plan) return; setBusy(true); setError('');
        try {const response = await ApplyCollectionDeletion(plan.token, keepAll, keepAll ? [] : plan.shelves.map((shelf) => new shelves.ShelfDecisions({shelfId: shelf.shelfId, decisions: decisions(choices[shelf.shelfId] ?? {})}))); if (response.error) throw new Error(response.error.message); onDeleted();}
        catch (error) {setError(error instanceof Error ? error.message : t('Collection could not be deleted.'));} finally {setBusy(false);}
    };
    return <ShelfDialog title={t('Delete “{0}”?', name)} busy={busy} onClose={onClose}>
        <p>{t('The Collection will be deleted after all linked Shelf decisions are approved. Library stories remain available.')}</p>
        {plan?.shelves.slice(shelfPage, shelfPage + 1).map((shelf) => <AffectedMembers key={`${plan.token}-${shelf.shelfId}`} plan={shelf} busy={busy} choices={choices[shelf.shelfId] ?? {}} onChange={(value) => setChoices({...choices, [shelf.shelfId]: value})}/>)}
        {plan && plan.shelves.length > 1 && <nav className="pagination" aria-label={t('Affected Shelf pages')}><button type="button" disabled={shelfPage === 0} onClick={() => setShelfPage(shelfPage - 1)}>{t('← Previous')}</button><span>{t('Page {0} of {1}', shelfPage + 1, plan.shelves.length)}</span><button type="button" disabled={shelfPage + 1 >= plan.shelves.length} onClick={() => setShelfPage(shelfPage + 1)}>{t('Next →')}</button></nav>}
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
        <button type="button" disabled={busy} onClick={() => void preview()}>{t('Refresh preview')}</button>
        <button type="button" disabled={busy || !plan} onClick={() => void apply(true)}>{t('Keep all Shelf stories and delete Collection')}</button>
        <button type="button" disabled={busy || !plan || !plan.shelves.every((shelf) => complete(shelf, choices[shelf.shelfId] ?? {}))} onClick={() => void apply(false)}>{t('Apply reviewed deletion')}</button>
    </ShelfDialog>;
}
