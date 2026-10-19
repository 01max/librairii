import {metadata} from '../wailsjs/go/models';

type Props = {
    status: metadata.CatalogStatus | null;
    busy: boolean;
    onSync: () => void;
    onDismiss: () => void;
};

export default function LuniiSync({status, busy, onSync, onDismiss}: Props) {
    const firstSync = status?.state === 'never_synced';
    const timestamp = status?.activatedAt || status?.fetchedAt;
    const date = timestamp ? new Date(timestamp) : null;
    const validDate = date && !Number.isNaN(date.valueOf());
    return (
        <section
            className={`lunii-sync${firstSync ? ' first-sync' : ''}`}
            aria-label="Official Lunii sync"
            data-state={firstSync ? 'first-lunii-sync' : 'lunii-sync-status'}
        >
            <div className="lunii-sync-copy">
                <h3>{firstSync ? 'Add covers and richer story details' :
                    status?.state === 'stale_cache' ? 'Using saved Lunii story details' :
                        'Lunii covers and story details'}</h3>
                <p>
                    Sync with the official Lunii API for richer descriptions and
                    cover images for matching stories.
                </p>
                <p className="last-sync">
                    Last successful sync: {validDate ? (
                        <time dateTime={timestamp}>{date.toLocaleString()}</time>
                    ) : firstSync ? 'Never' : 'Not available yet'}
                </p>
                {status?.state === 'stale_cache' && (
                    <p className="sync-warning" role="status">
                        {status.errorMessage || 'The latest sync failed. Saved Lunii details are still available.'}
                    </p>
                )}
            </div>
            <button type="button" disabled={busy} onClick={onSync}>
                {busy ? 'Syncing with Lunii…' : 'Sync with Lunii'}
            </button>
            <button type="button" className="notice-dismiss" aria-label="Close Lunii sync"
                onClick={onDismiss}>
                <span aria-hidden="true">×</span>
            </button>
        </section>
    );
}
