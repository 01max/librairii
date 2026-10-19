import {t} from './i18n';
import type {LibraryView} from './library-view';

const views = [
    {value: 'list', label: 'List', path: 'M3 5h18M3 12h18M3 19h18'},
    {value: 'small', label: 'Small covers', path: 'M3 3h4v4H3zM10 3h4v4h-4zM17 3h4v4h-4zM3 10h4v4H3zM10 10h4v4h-4zM17 10h4v4h-4zM3 17h4v4H3zM10 17h4v4h-4zM17 17h4v4h-4z'},
    {value: 'large', label: 'Large covers', path: 'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z'},
] as const;

export default function LibraryViewPicker({view, onChange}: {
    view: LibraryView;
    onChange: (view: LibraryView) => void;
}) {
    return (
        <div className="library-views" role="group" aria-label={t("Story view")}>
            {views.map(({value, label, path}) => (
                <button key={value} type="button" aria-pressed={view === value}
                    onClick={() => onChange(value)}>
                    <svg viewBox="0 0 24 24" aria-hidden="true"><path d={path}/></svg>
                    <span>{t(label)}</span>
                </button>
            ))}
        </div>
    );
}
