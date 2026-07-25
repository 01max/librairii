import {
    type CSSProperties,
    type FormEvent,
    useCallback,
    useEffect,
    useRef,
    useState,
} from 'react';
import {
    CreateTagDefinition,
    CreateTagValue,
    DeleteTagDefinition,
    DeleteTagValue,
    PlanTagDefinitionDeletion,
    PlanTagValueDeletion,
    RecolorTagDefinition,
    RenameTagDefinition,
    RenameTagValue,
    ReorderTagDefinitions,
    ReorderTagValues,
    TagCatalog,
} from '../wailsjs/go/main/App';
import {tagging} from '../wailsjs/go/models';

interface TagManagerProps {
    onClose: () => void;
    onCatalogChange: (catalog: tagging.Catalog) => void;
}

type DeletionTarget =
    | {kind: 'definition'; plan: tagging.DefinitionDeletionPlan}
    | {kind: 'value'; plan: tagging.ValueDeletionPlan};

function sourceLabel(definition: tagging.DefinitionWithValues): string {
    if (definition.source === 'builtin') {
        return 'Built-in';
    }
    if (definition.source === 'derived') {
        return 'System-derived';
    }
    return 'Custom';
}

function kindLabel(kind: string): string {
    return kind === 'choice' ? 'Choice' : 'Boolean';
}

function errorMessage(error?: {message: string}): string {
    return error?.message ?? 'The tag change could not be completed.';
}

export default function TagManager({onClose, onCatalogChange}: TagManagerProps) {
    const [catalog, setCatalog] = useState<tagging.Catalog | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const [deletion, setDeletion] = useState<DeletionTarget | null>(null);
    const initialFocus = useRef<HTMLInputElement>(null);

    const load = useCallback(async () => {
        const response = await TagCatalog();
        if (response.catalog) {
            setCatalog(response.catalog);
            onCatalogChange(response.catalog);
            setError(null);
        } else {
            setError(errorMessage(response.error));
        }
    }, [onCatalogChange]);

    useEffect(() => {
        const timer = window.setTimeout(() => void load(), 0);
        initialFocus.current?.focus();
        return () => window.clearTimeout(timer);
    }, [load]);

    useEffect(() => {
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                if (deletion) {
                    setDeletion(null);
                } else {
                    onClose();
                }
            }
        };
        window.addEventListener('keydown', closeOnEscape);
        return () => window.removeEventListener('keydown', closeOnEscape);
    }, [deletion, onClose]);

    async function run(action: () => Promise<{error?: {message: string}}>) {
        setBusy(true);
        setError(null);
        try {
            const response = await action();
            if (response.error) {
                setError(errorMessage(response.error));
                return false;
            }
            await load();
            return true;
        } catch {
            setError('The application could not be reached.');
            return false;
        } finally {
            setBusy(false);
        }
    }

    async function createDefinition(event: FormEvent<HTMLFormElement>) {
        event.preventDefault();
        const element = event.currentTarget;
        const form = new FormData(element);
        const created = await run(() => CreateTagDefinition(new tagging.CreateDefinition({
            key: String(form.get('key') ?? ''),
            label: String(form.get('label') ?? ''),
            color: String(form.get('color') ?? ''),
            kind: String(form.get('kind') ?? 'boolean'),
        })));
        if (created) {
            element.reset();
        }
    }

    async function saveDefinition(
        event: FormEvent<HTMLFormElement>,
        definition: tagging.DefinitionWithValues,
    ) {
        event.preventDefault();
        const element = event.currentTarget;
        const form = new FormData(element);
        const label = String(form.get('label') ?? '');
        const color = String(form.get('color') ?? '');
        const renamed = await run(() => RenameTagDefinition(definition.id, label));
        if (renamed && color !== definition.color) {
            await run(() => RecolorTagDefinition(definition.id, color));
        }
    }

    async function moveDefinition(index: number, offset: -1 | 1) {
        if (!catalog) {
            return;
        }
        const users = catalog.definitions.filter((definition) => !definition.protected);
        const destination = index + offset;
        if (destination < 0 || destination >= users.length) {
            return;
        }
        const ordered = users.map((definition) => definition.id);
        [ordered[index], ordered[destination]] = [ordered[destination], ordered[index]];
        await run(() => ReorderTagDefinitions(ordered));
    }

    async function planDefinitionDeletion(definitionID: number) {
        const response = await PlanTagDefinitionDeletion(definitionID);
        if (response.plan) {
            setDeletion({kind: 'definition', plan: response.plan});
            setError(null);
        } else {
            setError(errorMessage(response.error));
        }
    }

    async function createValue(
        event: FormEvent<HTMLFormElement>,
        definitionID: number,
    ) {
        event.preventDefault();
        const element = event.currentTarget;
        const form = new FormData(element);
        const created = await run(() => CreateTagValue(new tagging.CreateValue({
            definitionId: definitionID,
            key: String(form.get('key') ?? ''),
            label: String(form.get('label') ?? ''),
        })));
        if (created) {
            element.reset();
        }
    }

    async function renameValue(event: FormEvent<HTMLFormElement>, valueID: number) {
        event.preventDefault();
        const form = new FormData(event.currentTarget);
        await run(() => RenameTagValue(valueID, String(form.get('label') ?? '')));
    }

    async function moveValue(
        definition: tagging.DefinitionWithValues,
        index: number,
        offset: -1 | 1,
    ) {
        const destination = index + offset;
        if (destination < 0 || destination >= definition.values.length) {
            return;
        }
        const ordered = definition.values.map((value) => value.id);
        [ordered[index], ordered[destination]] = [ordered[destination], ordered[index]];
        await run(() => ReorderTagValues(definition.id, ordered));
    }

    async function planValueDeletion(valueID: number) {
        const response = await PlanTagValueDeletion(valueID);
        if (response.plan) {
            setDeletion({kind: 'value', plan: response.plan});
            setError(null);
        } else {
            setError(errorMessage(response.error));
        }
    }

    async function confirmDeletion() {
        if (!deletion) {
            return;
        }
        const deleted = deletion.kind === 'definition'
            ? await run(() => DeleteTagDefinition(deletion.plan))
            : await run(() => DeleteTagValue(deletion.plan));
        if (deleted) {
            setDeletion(null);
        }
    }

    const userDefinitions = catalog?.definitions.filter(
        (definition) => !definition.protected,
    ) ?? [];

    return (
        <div className="dialog-backdrop tag-manager-backdrop">
            <section
                className="tag-manager"
                role="dialog"
                aria-modal="true"
                aria-labelledby="tag-manager-title"
            >
                <header className="tag-manager-head">
                    <div>
                        <div className="dialog-kicker">Library structure</div>
                        <h2 id="tag-manager-title">Manage tags</h2>
                        <p>Define reusable boolean flags and ordered choices for your stories.</p>
                    </div>
                    <button type="button" onClick={onClose} aria-label="Close tag manager">×</button>
                </header>

                {error && <p className="dialog-error" role="alert">{error}</p>}

                <form className="new-tag" onSubmit={(event) => void createDefinition(event)}>
                    <h3>New tag</h3>
                    <label>
                        Key
                        <input ref={initialFocus} name="key" placeholder="reading-level"/>
                    </label>
                    <label>
                        Label
                        <input name="label" placeholder="Reading level"/>
                    </label>
                    <label>
                        Color
                        <input name="color" type="color" defaultValue="#405cf5"/>
                    </label>
                    <label>
                        State
                        <select name="kind" defaultValue="boolean">
                            <option value="boolean">Boolean</option>
                            <option value="choice">Choice</option>
                        </select>
                    </label>
                    <button className="primary" type="submit" disabled={busy}>Add tag</button>
                </form>

                <div className="tag-definition-list">
                    {!catalog && !error && <p className="tag-loading">Loading tags…</p>}
                    {catalog?.definitions.map((definition) => {
                        const userIndex = userDefinitions.findIndex(
                            (candidate) => candidate.id === definition.id,
                        );
                        return (
                            <article className="tag-definition" key={definition.id}>
                                <div className="tag-identity">
                                    <i
                                        style={{'--tag-color': definition.color} as CSSProperties}
                                        aria-hidden="true"
                                    />
                                    <div>
                                        <b>{definition.label}</b>
                                        <code>{definition.key}</code>
                                    </div>
                                    <span>
                                        {sourceLabel(definition)} · {kindLabel(definition.kind)}
                                        {definition.protected ? ' · Protected' : ''}
                                    </span>
                                </div>

                                {definition.protected ? (
                                    <p className="protected-note">
                                        Managed by Librairii and shown with a text label as well as color.
                                    </p>
                                ) : (
                                    <>
                                        <form
                                            className="tag-edit"
                                            onSubmit={(event) => void saveDefinition(event, definition)}
                                        >
                                            <label>
                                                Label
                                                <input name="label" defaultValue={definition.label}/>
                                            </label>
                                            <label>
                                                Color
                                                <input
                                                    name="color"
                                                    type="color"
                                                    defaultValue={definition.color}
                                                />
                                            </label>
                                            <button type="submit" disabled={busy}>Save</button>
                                            <button
                                                type="button"
                                                aria-label={`Move ${definition.label} up`}
                                                disabled={busy || userIndex === 0}
                                                onClick={() => void moveDefinition(userIndex, -1)}
                                            >
                                                ↑
                                            </button>
                                            <button
                                                type="button"
                                                aria-label={`Move ${definition.label} down`}
                                                disabled={busy || userIndex === userDefinitions.length - 1}
                                                onClick={() => void moveDefinition(userIndex, 1)}
                                            >
                                                ↓
                                            </button>
                                            <button
                                                className="text-danger"
                                                type="button"
                                                aria-label={`Delete ${definition.label}`}
                                                disabled={busy}
                                                onClick={() => void planDefinitionDeletion(definition.id)}
                                            >
                                                Delete
                                            </button>
                                        </form>

                                        {definition.kind === 'choice' && (
                                            <div className="tag-values">
                                                <h4>Choice values</h4>
                                                {definition.values.map((value, index) => (
                                                    <form
                                                        className="tag-value"
                                                        key={value.id}
                                                        onSubmit={(event) => void renameValue(event, value.id)}
                                                    >
                                                        <code>{value.key}</code>
                                                        <label>
                                                            <span className="sr-only">
                                                                {`Label for ${value.key}`}
                                                            </span>
                                                            <input name="label" defaultValue={value.label}/>
                                                        </label>
                                                        <button type="submit" disabled={busy}>Save</button>
                                                        <button
                                                            type="button"
                                                            aria-label={`Move ${value.label} up`}
                                                            disabled={busy || index === 0}
                                                            onClick={() => void moveValue(definition, index, -1)}
                                                        >
                                                            ↑
                                                        </button>
                                                        <button
                                                            type="button"
                                                            aria-label={`Move ${value.label} down`}
                                                            disabled={busy || index === definition.values.length - 1}
                                                            onClick={() => void moveValue(definition, index, 1)}
                                                        >
                                                            ↓
                                                        </button>
                                                        <button
                                                            className="text-danger"
                                                            type="button"
                                                            aria-label={`Delete ${value.label}`}
                                                            disabled={busy}
                                                            onClick={() => void planValueDeletion(value.id)}
                                                        >
                                                            Delete
                                                        </button>
                                                    </form>
                                                ))}
                                                <form
                                                    className="new-value"
                                                    onSubmit={(event) => void createValue(event, definition.id)}
                                                >
                                                    <input name="key" aria-label={`${definition.label} value key`} placeholder="key"/>
                                                    <input name="label" aria-label={`${definition.label} value label`} placeholder="Label"/>
                                                    <button type="submit" disabled={busy}>Add value</button>
                                                </form>
                                            </div>
                                        )}
                                    </>
                                )}
                            </article>
                        );
                    })}
                </div>
            </section>

            {deletion && (
                <div className="dialog-backdrop confirmation-backdrop">
                    <section
                        className="detail-dialog"
                        role="alertdialog"
                        aria-modal="true"
                        aria-labelledby="tag-delete-title"
                    >
                        <div className="dialog-kicker">Confirm impact</div>
                        <h3 id="tag-delete-title">
                            Delete {deletion.kind === 'definition'
                                ? deletion.plan.definition.label
                                : deletion.plan.value.label}?
                        </h3>
                        <div className="removal-confirmation">
                            <b>This removes saved metadata.</b>
                            <p>
                                {deletion.kind === 'definition'
                                    ? `${deletion.plan.assignmentCount} assignments, ${deletion.plan.valueCount} values, and ${deletion.plan.affectedShelfCount} saved shelves are affected.`
                                    : `${deletion.plan.assignmentCount} assignments and ${deletion.plan.affectedShelfCount} saved shelves are affected.`}
                            </p>
                        </div>
                        <div className="dialog-actions">
                            <button type="button" onClick={() => setDeletion(null)}>Cancel</button>
                            <button
                                className="danger"
                                type="button"
                                disabled={busy}
                                onClick={() => void confirmDeletion()}
                            >
                                Delete permanently
                            </button>
                        </div>
                    </section>
                </div>
            )}
        </div>
    );
}
