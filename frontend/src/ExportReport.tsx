import {operations} from '../wailsjs/go/models';

const exportReportGroups: Array<{
    status: string;
    label: string;
}> = [
    {status: 'succeeded', label: 'Exported'},
    {status: 'skipped', label: 'Skipped'},
    {status: 'conflicted', label: 'Conflicted'},
    {status: 'failed', label: 'Failed'},
    {status: 'cancelled', label: 'Cancelled'},
];

export default function ExportReport({
    operation,
    onReveal,
}: {
    operation: operations.Snapshot;
    onReveal: (operationID: string) => void;
}) {
    return (
        <section className="export-report" aria-label="Export report">
            <div className="export-report-context">
                <span>
                    Destination · {operation.destination || 'Selected folder'}
                </span>
                {operation.sourceShelfNames && operation.sourceShelfNames.length > 0 && (
                    <span>
                        Source shelves · {operation.sourceShelfNames.join(', ')}
                    </span>
                )}
            </div>
            <div className="export-report-groups">
                {exportReportGroups.map((group) => {
                    const items = operation.items.filter(
                        (item) => item.status === group.status,
                    );
                    if (items.length === 0) {
                        return null;
                    }
                    return (
                        <section
                            className={`export-report-group ${group.status}`}
                            key={group.status}
                        >
                            <h4>{group.label} · {items.length}</h4>
                            <ul>
                                {items.map((item) => (
                                    <li key={item.id}>
                                        <b>
                                            {item.storyTitle ||
                                                item.outputName ||
                                                item.sourceName}
                                        </b>
                                        <span>
                                            {item.outcomeMessage ||
                                                item.outputName ||
                                                item.status}
                                        </span>
                                    </li>
                                ))}
                            </ul>
                        </section>
                    );
                })}
            </div>
            <button type="button" onClick={() => onReveal(operation.id)}>
                Reveal destination
            </button>
        </section>
    );
}

