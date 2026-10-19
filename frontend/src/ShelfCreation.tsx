import {useState} from 'react';
import {PreviewShelfCreation, CreateShelfFromReview} from '../wailsjs/go/main/App';
import {collections, shelves} from '../wailsjs/go/models';
import {t} from './i18n';
import ShelfDialog from './ShelfDialog';
import ReviewWarnings from './ReviewWarnings';
import OutcomeText from './OutcomeText';

export default function ShelfCreation({collections: sources, manualUUIDs, onClose, onCreated}: {
    collections: collections.Summary[]; manualUUIDs: string[]; onClose: () => void; onCreated: (id: number) => void;
}) {
    const [name, setName] = useState('');
    const [ids, setIDs] = useState<number[]>([]);
    const [review, setReview] = useState<shelves.Review | null>(null);
    const [selected, setSelected] = useState<string[]>([]);
    const [page, setPage] = useState(0);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState('');
    const preview = async () => {
        setBusy(true); setError('');
        try {
            const response = await PreviewShelfCreation(new shelves.CreationRequest({name, manualUuids: manualUUIDs, collectionIds: ids}));
            if (!response.review) throw new Error(response.error?.message ?? t('Could not preview Shelf.'));
            setReview(response.review); setSelected([]); setPage(0);
        } catch (error) { setError(error instanceof Error ? error.message : t('Could not preview Shelf.')); }
        finally { setBusy(false); }
    };
    const create = async () => {
        if (!review) return;
        setBusy(true); setError('');
        try {
            const response = await CreateShelfFromReview(review.token, selected);
            if (!response.shelf) throw new Error(response.error?.message ?? t('Could not create Shelf.'));
            onCreated(response.shelf.id);
        } catch (error) { setError(error instanceof Error ? error.message : t('Could not create Shelf.')); }
        finally { setBusy(false); }
    };
    return <ShelfDialog title={t('Create Shelf')} busy={busy} onClose={onClose}>
        <label>{t('Shelf name')}<input value={name} disabled={busy || !!review} onChange={(event) => setName(event.target.value)}/></label>
        {!review ? <>
            <fieldset disabled={busy}><legend>{t('Suggest stories matching any of these Collections')}</legend>
                {sources.map((source) => <label key={source.id}><input type="checkbox" checked={ids.includes(source.id)}
                    onChange={() => setIDs(ids.includes(source.id) ? ids.filter((id) => id !== source.id) : [...ids, source.id])}/>{source.name}</label>)}
            </fieldset>
            <p>{t('{0} manually selected stories available for preview.', manualUUIDs.length)}</p>
            <button type="button" disabled={busy || !name.trim()} onClick={() => void preview()}>{t('Preview initial stories')}</button>
        </> : <>
            {review.incomplete && <p role="status">{t('Review incomplete. Some sources could not be evaluated.')}</p>}
            <ReviewWarnings warnings={review.warnings ?? []}/>
            <p>{t('Choose initial members explicitly. Unchecked stories remain suggestions.')}</p>
            <button type="button" disabled={busy} onClick={() => setSelected(review.additions.map((item) => item.uuid))}>{t('Select all candidates')}</button>
            <button type="button" disabled={busy} onClick={() => setSelected([])}>{t('Clear selection')}</button>
            <ul className="curated-candidates">{review.additions.slice(page * 25, (page + 1) * 25).map((item) => <li key={item.uuid}>
                <label><input type="checkbox" disabled={busy} checked={selected.includes(item.uuid)} onChange={() => setSelected(selected.includes(item.uuid) ? selected.filter((id) => id !== item.uuid) : [...selected, item.uuid])}/>
                    {item.title}{item.story?.broken ? ` · ${t('Broken')}` : ''}</label>
                <small>{item.manual ? t('Manual selection · protected') : t('Matches: {0}', (item.sources ?? []).map((source) => source.name).join(', '))}</small>
            </li>)}</ul>
            <div className="curated-actions"><button type="button" disabled={page === 0} onClick={() => setPage(page - 1)}>{t('← Previous')}</button>
                <span>{t('Page {0} of {1}', page + 1, Math.max(1, Math.ceil(review.additions.length / 25)))}</span>
                <button type="button" disabled={(page + 1) * 25 >= review.additions.length} onClick={() => setPage(page + 1)}>{t('Next →')}</button></div>
            <button type="button" disabled={busy} onClick={() => {setReview(null); setSelected([]);}}>{t('Change inputs')}</button>
            <button type="button" disabled={busy} onClick={() => void preview()}>{t('Refresh preview')}</button>
            <button type="button" disabled={busy} onClick={() => void create()}>{t(selected.length === 1 ? 'Create with {0} story' : 'Create with {0} stories', selected.length)}</button>
        </>}
        {error && <p role="alert" className="dialog-error"><OutcomeText text={error}/></p>}
    </ShelfDialog>;
}
