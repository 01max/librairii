import OutcomeText from './OutcomeText';
import {t, currentLanguage} from './i18n';
import {useEffect, useMemo, useRef, useState} from 'react';
import {OperationHistory} from '../wailsjs/go/main/App';
import {operations} from '../wailsjs/go/models';
import ExportReport from './ExportReport';
import ModalBackdrop from './ModalBackdrop';
import {useModalFocus} from './modal-focus';
import {operationIsTerminal} from './import-state';

const kindLabels: Record<string, string> = {
    import: 'Import', metadata_sync: 'Lunii sync', export: 'Export',
};
const statusLabels: Record<string, string> = {
    queued: 'Queued', running: 'In progress', succeeded: 'Completed',
    partially_succeeded: 'Completed with exceptions', failed: 'Failed',
    cancelled: 'Cancelled', interrupted: 'Interrupted',
};

export default function OperationHistoryDialog({liveOperations, onClose, onReveal}: {
    liveOperations: operations.Snapshot[];
    onClose: () => void;
    onReveal: (operationID: string) => Promise<string | null>;
}) {
    const dialog = useRef<HTMLElement>(null);
    const close = useRef<HTMLButtonElement>(null);
    const [entries, setEntries] = useState<operations.Snapshot[]>([]);
    const [nextBefore, setNextBefore] = useState('');
    const [request, setRequest] = useState({before: '', revision: 0});
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [revealError, setRevealError] = useState<string | null>(null);
    useModalFocus(dialog, close);

    useEffect(() => {
        const dismiss = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.preventDefault();
                onClose();
            }
        };
        window.addEventListener('keydown', dismiss);
        return () => window.removeEventListener('keydown', dismiss);
    }, [onClose]);

    useEffect(() => {
        let active = true;
        void OperationHistory(request.before).then((response) => {
            if (!active) return;
            if (!response.page) {
                setError(response.error?.message ?? t("History could not be loaded. Try again."));
                return;
            }
            const page = response.page;
            setEntries((current) => request.before
                ? [...new Map([...current, ...page.operations].map((entry) => [entry.id, entry])).values()]
                : page.operations);
            setNextBefore(page.nextBefore ?? '');
        }).catch(() => {
            if (active) setError(t("History could not be loaded. Try again."));
        }).finally(() => {
            if (active) setLoading(false);
        });
        return () => { active = false; };
    }, [request]);

    function loadPage(before: string) {
        setLoading(true);
        setError(null);
        setRequest((current) => ({before, revision: current.revision + 1}));
    }

    const history = useMemo(() => {
        const byID = new Map(entries.map((entry) => [entry.id, entry]));
        for (const entry of liveOperations) {
            const saved = byID.get(entry.id);
            if (saved && operationIsTerminal(saved) && !operationIsTerminal(entry)) continue;
            byID.set(entry.id, entry);
        }
        return [...byID.values()].sort((left, right) =>
            right.createdAt.localeCompare(left.createdAt) || right.id.localeCompare(left.id),
        );
    }, [entries, liveOperations]);

    return (
        <ModalBackdrop onDismiss={onClose} className="history-backdrop">
            <section ref={dialog} className="detail-dialog operation-history" role="dialog"
                aria-modal="true" aria-labelledby="operation-history-title">
                <header className="history-header">
                    <div>
                        <h3 id="operation-history-title">{t("History")}</h3>
                        <p>{t("Imports, Lunii syncs, and exports saved in your library.")}</p>
                    </div>
                    <button ref={close} type="button" onClick={onClose} aria-label={t("Close history")}>
                        {t("Close")}<span aria-hidden="true">×</span>
                    </button>
                </header>
                <div className="history-entries" aria-busy={loading}>
                    {history.map((operation) => (
                        <details className="history-entry" key={operation.id}>
                            <summary>
                                <span className="history-kind">{t(kindLabels[operation.kind] ?? operation.kind)}</span>
                                <span className={`history-status ${operation.status}`}>
                                    {t(statusLabels[operation.status] ?? operation.status)}
                                </span>
                                <time dateTime={operation.createdAt}>
                                    {new Date(operation.createdAt).toLocaleString(currentLanguage())}
                                </time>
                            </summary>
                            <div className="history-detail">
                                <p>{operation.completedItems} {' ' + t("of") + ' '}{operation.totalItems} {' ' + t("items finished.")}</p>
                                {operation.errorMessage && <p>{<OutcomeText text={operation.errorMessage}/>}</p>}
                                {operation.kind === 'export' && operationIsTerminal(operation) ? (
                                    <ExportReport operation={operation} onReveal={(operationID) => {
                                        setRevealError(null);
                                        void onReveal(operationID).then(setRevealError);
                                    }}/>
                                ) : (
                                    <ul className="operation-items">
                                        {operation.items.map((item) => (
                                            <li key={item.id}>
                                                <b>{item.storyTitle || item.sourceName}</b>
                                                <span>{<OutcomeText text={item.outcomeMessage || statusLabels[item.status] || item.status}/>}</span>
                                            </li>
                                        ))}
                                    </ul>
                                )}
                            </div>
                        </details>
                    ))}
                    {!loading && !error && history.length === 0 && (
                        <p className="history-empty">{t("No activity yet. Import stories or sync with Lunii to get started.")}</p>
                    )}
                    {loading && <p role="status">{t("Loading history…")}</p>}
                    {error && <p role="alert">{error}</p>}
                    {revealError && <p role="alert">{revealError}</p>}
                </div>
                <footer className="history-actions">
                    <button type="button" disabled={loading}
                        onClick={() => loadPage('')}>
                        {t("Refresh history")}</button>
                    {error ? (
                        <button type="button" disabled={loading}
                            onClick={() => loadPage(request.before)}>
                            {t("Try again")}</button>
                    ) : nextBefore && (
                        <button type="button" disabled={loading}
                            onClick={() => loadPage(nextBefore)}>
                            {t("Load older activity")}</button>
                    )}
                </footer>
            </section>
        </ModalBackdrop>
    );
}
