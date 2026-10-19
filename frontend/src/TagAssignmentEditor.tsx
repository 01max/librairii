import OutcomeText from './OutcomeText';
import {isStoryTag, systemLabel, systemValue} from './system-label';
import {t} from './i18n';
import ModalBackdrop from './ModalBackdrop';
import {
    type ChangeEvent,
    type CSSProperties,
    useCallback,
    useEffect,
    useRef,
    useState,
} from 'react';
import {
    SetBooleanTag,
    SetChoiceTagValue,
    TagAssignmentWorkspace,
} from '../wailsjs/go/main/App';
import {tagging} from '../wailsjs/go/models';
import {useModalFocus} from './modal-focus';

interface TagAssignmentEditorProps {
    storyIDs: number[];
    onClose: () => void;
    onWorkspaceChange: (workspace: tagging.AssignmentWorkspace) => void;
    onAssignmentsChange: () => Promise<void>;
}

interface MixedCheckboxProps {
    checked: boolean;
    mixed: boolean;
    disabled?: boolean;
    label: string;
    onChange: (checked: boolean) => void;
}

function MixedCheckbox({
    checked,
    mixed,
    disabled,
    label,
    onChange,
}: MixedCheckboxProps) {
    const ref = useRef<HTMLInputElement>(null);
    useEffect(() => {
        if (ref.current) {
            ref.current.indeterminate = mixed;
        }
    }, [mixed]);
    return (
        <label className="assignment-option">
            <input
                ref={ref}
                type="checkbox"
                checked={checked}
                disabled={disabled}
                aria-checked={mixed ? 'mixed' : checked}
                onChange={(event: ChangeEvent<HTMLInputElement>) => (
                    onChange(event.currentTarget.checked)
                )}
            />
            {label}
            {mixed && <span>{t("Mixed")}</span>}
        </label>
    );
}

export default function TagAssignmentEditor({
    storyIDs,
    onClose,
    onWorkspaceChange,
    onAssignmentsChange,
}: TagAssignmentEditorProps) {
    const [workspace, setWorkspace] = useState<tagging.AssignmentWorkspace | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [notice, setNotice] = useState<string | null>(null);
    const dialog = useRef<HTMLElement>(null);
    const initialFocus = useRef<HTMLButtonElement>(null);
    useModalFocus(dialog, initialFocus);

    const load = useCallback(async () => {
        const response = await TagAssignmentWorkspace(storyIDs);
        if (response.workspace) {
            setWorkspace(response.workspace);
            onWorkspaceChange(response.workspace);
            setError(null);
        } else {
            setError(response.error?.message ?? t("Story tags could not be loaded."));
        }
    }, [onWorkspaceChange, storyIDs]);

    useEffect(() => {
        const timer = window.setTimeout(() => void load(), 0);
        return () => window.clearTimeout(timer);
    }, [load]);

    useEffect(() => {
        const closeOnEscape = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                onClose();
            }
        };
        window.addEventListener('keydown', closeOnEscape);
        return () => window.removeEventListener('keydown', closeOnEscape);
    }, [onClose]);

    async function apply(
        request: () => Promise<{
            result?: tagging.AssignmentResult;
            error?: {message: string};
        }>,
    ) {
        setBusy(true);
        setError(null);
        setNotice(null);
        try {
            const response = await request();
            if (!response.result) {
                setError(response.error?.message ?? t("Story tags could not be updated."));
                return;
            }
            setNotice(response.result.changedStories === 0
                ? 'No stories changed; this tag already matched the selection.'
                : t("{0} of {1} selected stories updated.", response.result.changedStories, response.result.requestedStories));
            await load();
            await onAssignmentsChange();
        } catch {
            setError(t("The application could not be reached."));
        } finally {
            setBusy(false);
        }
    }

    const states = new Map(
        workspace?.states.map((state) => [state.definitionId, state]) ?? [],
    );

    return (
        <ModalBackdrop onDismiss={onClose} disabled={busy}>
            <section
                ref={dialog}
                className="assignment-editor"
                role="dialog"
                aria-modal="true"
                aria-labelledby="assignment-title"
            >
                <header className="tag-manager-head">
                    <div>
                        <div className="dialog-kicker">{t("Story metadata")}</div>
                        <h2 id="assignment-title">
                            {storyIDs.length === 1
                                ? t("Edit story tags")
                                : t("Edit tags for {0} stories", storyIDs.length)}
                        </h2>
                        <p>{t("Mixed means the tag is present on only part of this selection.")}</p>
                    </div>
                    <button
                        ref={initialFocus}
                        type="button"
                        onClick={onClose}
                        aria-label={t("Close story tags")}
                    >
                        ×
                    </button>
                </header>

                {error && <p className="dialog-error" role="alert">{<OutcomeText text={error}/> }</p>}
                {notice && <p className="assignment-notice" role="status">{notice}</p>}

                <div className="assignment-list">
                    {workspace?.catalog.definitions.filter(isStoryTag).map((definition) => {
                        const state = states.get(definition.id);
                        const derived = definition.source === 'derived';
                        const assigned = state?.assignedStories ?? 0;
                        const all = assigned === workspace.requestedStories;
                        const mixed = assigned > 0 && !all;
                        return (
                            <section
                                className="assignment-definition"
                                key={definition.id}
                            >
                                <div className="assignment-definition-head">
                                    <i
                                        style={{'--tag-color': definition.color} as CSSProperties}
                                        aria-hidden="true"
                                    />
                                    <div>
                                        <b>{systemLabel(definition)}</b>
                                        <span>
                                            {definition.source === 'derived'
                                                ? t("System-derived")
                                                : definition.source === 'builtin'
                                                    ? t("Built-in")
                                                    : t("Custom")}
                                            {' · '}
                                            {definition.kind === 'choice' ? t("Choice") : t("Boolean")}
                                        </span>
                                    </div>
                                </div>
                                {derived ? (
                                    <>
                                        <p className="protected-note">
                                            {t("Derived tags are read-only. Use them to inspect and filter stories.")}</p>
                                        <div className="assignment-values derived-values">
                                            {definition.values.flatMap((value) => {
                                                const valueState = state?.values.find(
                                                    (candidate) => candidate.valueId === value.id,
                                                );
                                                if (!valueState || valueState.assignedStories === 0) {
                                                    return [];
                                                }
                                                const mixedValue =
                                                    valueState.assignedStories <
                                                    workspace.requestedStories;
                                                return [(
                                                    <span className="tag" key={value.id}>
                                                        {systemLabel(definition)} · {systemValue(definition,value)}
                                                        {t(" · System-derived")}
                                                        {mixedValue ? ' · Mixed' : ''}
                                                    </span>
                                                )];
                                            })}
                                        </div>
                                    </>
                                ) : definition.kind === 'boolean' ? (
                                    <MixedCheckbox
                                        checked={all}
                                        mixed={mixed}
                                        disabled={busy}
                                        label={systemLabel(definition)}
                                        onChange={(checked) => void apply(() => (
                                            SetBooleanTag(storyIDs, definition.id, checked)
                                        ))}
                                    />
                                ) : (
                                    <div className="assignment-values">
                                        {definition.values.map((value) => {
                                            const valueState = state?.values.find(
                                                (candidate) => candidate.valueId === value.id,
                                            );
                                            const count = valueState?.assignedStories ?? 0;
                                            const valueAll = count === workspace.requestedStories;
                                            return (
                                                <MixedCheckbox
                                                    key={value.id}
                                                    checked={valueAll}
                                                    mixed={count > 0 && !valueAll}
                                                    disabled={busy}
                                                    label={systemValue(definition,value)}
                                                    onChange={(checked) => void apply(() => (
                                                        SetChoiceTagValue(
                                                            storyIDs,
                                                            definition.id,
                                                            value.id,
                                                            checked,
                                                        )
                                                    ))}
                                                />
                                            );
                                        })}
                                    </div>
                                )}
                            </section>
                        );
                    })}
                </div>
                <div className="dialog-actions">
                    <button type="button" onClick={onClose}>{t("Done")}</button>
                </div>
            </section>
        </ModalBackdrop>
    );
}
