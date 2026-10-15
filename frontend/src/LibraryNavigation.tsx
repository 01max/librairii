type Props = {
    libraryActive: boolean;
    disabled: boolean;
    importing: boolean;
    exporting: boolean;
    onLibrary: () => void;
    onTags: () => void;
    onImport: () => void;
    onExport: () => void;
};

export default function LibraryNavigation({
    libraryActive, disabled, importing, exporting,
    onLibrary, onTags, onImport, onExport,
}: Props) {
    return (
        <nav aria-label="Primary navigation">
            <button
                className={libraryActive ? 'active' : undefined}
                type="button"
                aria-current={libraryActive ? 'page' : undefined}
                disabled={disabled}
                onClick={onLibrary}
                title="Show all stories"
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M4 4h4v16H4zM10 4h4v16h-4zM16 5l3-1 4 15-3 1z"/>
                </svg>
                <span>Library</span>
            </button>
            <button type="button" disabled={disabled} onClick={onTags} title="Manage story tags">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M3 3h8l10 10-8 8L3 11z"/><circle cx="7" cy="7" r="1"/>
                </svg>
                <span>Tags</span>
            </button>
            <button
                type="button" disabled={disabled || importing}
                onClick={onImport} title="Import story archives"
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>
                </svg>
                <span>Import</span>
            </button>
            <button
                type="button" aria-label="Export current result"
                disabled={disabled || exporting}
                onClick={onExport} title="Export the stories in the current view"
            >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M12 15V3m-5 5 5-5 5 5M4 16v5h16v-5"/>
                </svg>
                <span>Export</span>
            </button>
        </nav>
    );
}
