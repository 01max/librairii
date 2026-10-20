import {type CSSProperties, useEffect, useId, useRef, useState} from 'react';
import type {tagging} from '../wailsjs/go/models';
import {type BooleanFilterState, type CollectionQuery} from './query-codec';
import {isStoryTag, systemLabel, systemValue} from './system-label';
import {t} from './i18n';

export default function TagSearchFilters({catalog, query, onChange}: {
    catalog?: tagging.Catalog | null; query: CollectionQuery; onChange: (query: CollectionQuery) => void;
}) {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState('');
    const panelID = useId();
    const root = useRef<HTMLDivElement>(null);
    const opener = useRef<HTMLButtonElement>(null);
    const searchInput = useRef<HTMLInputElement>(null);
    useEffect(() => {
        if (!open) return;
        searchInput.current?.focus();
        const outside = (event: PointerEvent) => {if (!root.current?.contains(event.target as Node)) setOpen(false);};
        const escape = (event: KeyboardEvent) => {if (event.key === 'Escape') {event.stopImmediatePropagation(); setOpen(false); opener.current?.focus();}};
        document.addEventListener('pointerdown', outside);
        document.addEventListener('keydown', escape, true);
        return () => {document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape, true);};
    }, [open]);
    const close = () => {setOpen(false); opener.current?.focus();};
    const definitions = catalog?.definitions.filter(isStoryTag) ?? [];
    const visible = definitions.filter((definition) => [systemLabel(definition), ...definition.values.map((value) => systemValue(definition, value))]
        .some((text) => text.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase())));
    const count = query.booleanFilters.length + query.choiceFilters.reduce((total, filter) => total + filter.valueIds.length, 0);
    const boolean = (definitionId: number, state: BooleanFilterState) => onChange({...query, page: 1,
        booleanFilters: [...query.booleanFilters.filter((filter) => filter.definitionId !== definitionId), ...(state === 'ignored' ? [] : [{definitionId, state}])],
    });
    const choice = (definitionId: number, valueId: number) => {
        const current = query.choiceFilters.find((filter) => filter.definitionId === definitionId)?.valueIds ?? [];
        const valueIds = current.includes(valueId) ? current.filter((id) => id !== valueId) : [...current, valueId];
        onChange({...query, page: 1, choiceFilters: [...query.choiceFilters.filter((filter) => filter.definitionId !== definitionId), ...(valueIds.length ? [{definitionId, valueIds}] : [])]});
    };
    return <div className="search-tag-filter" ref={root} onKeyDown={(event) => {
        if (event.key === 'Escape' && open) {event.stopPropagation(); close();}
    }}>
        <button ref={opener} type="button" className="tag-filter-trigger" aria-expanded={open} aria-controls={panelID}
            onClick={() => {setSearch(''); setOpen(!open);}}>{t('Filter by tags')}{count > 0 && <span aria-hidden="true">{count}</span>}</button>
        {open && <section id={panelID} className="tag-filter-panel" aria-label={t('Tag filters')}>
            <input ref={searchInput} type="search" aria-label={t('Search tags')} placeholder={t('Search tags')} value={search} onChange={(event) => setSearch(event.currentTarget.value)}/>
            <div className="tag-filter-options">
                {!catalog ? <p>{t('Loading tags…')}</p> : !definitions.length ? <p>{t('No tags yet.')}</p> : !visible.length && <p>{t('No matching tags.')}</p>}
                {visible.map((definition) => <fieldset key={definition.id}>
                    <legend><i style={{'--c': definition.color} as CSSProperties} aria-hidden="true"/>{systemLabel(definition)}</legend>
                    {definition.kind === 'boolean' ? <label>{t('Match')}
                        <select aria-label={t('{0} filter', systemLabel(definition))} value={query.booleanFilters.find((filter) => filter.definitionId === definition.id)?.state ?? 'ignored'}
                            onChange={(event) => boolean(definition.id, event.currentTarget.value as BooleanFilterState)}>
                            <option value="ignored">{t('Any')}</option><option value="true">{t('Tagged')}</option><option value="false">{t('Not tagged')}</option>
                        </select></label> : definition.values.map((value) => <label key={value.id}>
                            <input type="checkbox" checked={query.choiceFilters.some((filter) => filter.definitionId === definition.id && filter.valueIds.includes(value.id))} onChange={() => choice(definition.id, value.id)}/>
                            {systemValue(definition, value)}</label>)}
                </fieldset>)}
            </div>
            <div className="tag-filter-actions"><button type="button" disabled={!count} onClick={() => onChange({...query, booleanFilters: [], choiceFilters: [], page: 1})}>{t('Clear tag filters')}</button>
                <button type="button" onClick={close}>{t('Done')}</button></div>
        </section>}
    </div>;
}
