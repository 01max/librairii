import BrokenToggle from './BrokenToggle';
import OutcomeText from './OutcomeText';
import {isStoryTag, systemLabel, systemValue} from './system-label';
import {t} from './i18n';
import ModalBackdrop from './ModalBackdrop';
import {type CSSProperties, useEffect, useMemo, useRef, useState} from 'react';
import {library, tagging} from '../wailsjs/go/models';
import {TagAssignmentWorkspace} from '../wailsjs/go/main/App';
import {useModalFocus} from './modal-focus';
import {readableStoryDescription} from './story-description';

type Props = {
    story: library.StorySummary;
    archive?: library.StoryDetail['archive'];
    artworkURL?: string;
    coverStyle?: CSSProperties;
    verification: string;
    removing: boolean;
    error: string | null;
    tagSelectionCount: number;
    flagging: boolean;
    flagError: string | null;
    flagNotice: string | null;
    canUndoFlag: boolean;
    onFlagChange: (broken: boolean) => void;
    onUndoFlag: () => void;
    onEditTags: () => void;
    onClose: () => void;
    onRemove: () => void;
};

export default function StoryDetailsDialog({
    story, archive, artworkURL, coverStyle, verification,
    removing, error, tagSelectionCount, onEditTags, onClose, onRemove,
    flagging, flagError, flagNotice, canUndoFlag, onFlagChange, onUndoFlag,
}: Props) {
    const [tagWorkspace, setTagWorkspace] = useState<tagging.AssignmentWorkspace | null>(null);
    const [tagError, setTagError] = useState(false);
    useEffect(() => {
        let active = true;
        void TagAssignmentWorkspace([story.id]).then((response) => {
            if (active) {
                setTagWorkspace(response.workspace ?? null);
                setTagError(!response.workspace);
            }
        }).catch(() => { if (active) setTagError(true); });
        return () => { active = false; };
    }, [story.id]);
    const tags = tagWorkspace?.catalog.definitions.filter(isStoryTag).flatMap((definition) => {
        const state = tagWorkspace.states.find((candidate) => candidate.definitionId === definition.id);
        if (!state) return [];
        if (definition.kind === 'boolean') {
            return state.assignedStories > 0
                ? [{key: `${definition.id}`, label: systemLabel(definition), color: definition.color}]
                : [];
        }
        return definition.values.filter((value) => state.values.some(
            (assigned) => assigned.valueId === value.id && assigned.assignedStories > 0,
        )).map((value) => ({key: `${definition.id}-${value.id}`, label: `${systemLabel(definition)} · ${systemValue(definition,value)}`, color: definition.color}));
    }) ?? [];
    const [technicalOpen, setTechnicalOpen] = useState(false);
    const [confirming, setConfirming] = useState(false);
    const [failedArtworkURL, setFailedArtworkURL] = useState<string>();
    const dialog = useRef<HTMLElement>(null);
    const closeButton = useRef<HTMLButtonElement>(null);
    const keepButton = useRef<HTMLButtonElement>(null);
    const removeButton = useRef<HTMLButtonElement>(null);
    const wasConfirming = useRef(false);
    const description = useMemo(
        () => readableStoryDescription(story.description),
        [story.description],
    );
    const hasArtwork = artworkURL && failedArtworkURL !== artworkURL;
    useModalFocus(dialog, closeButton);

    useEffect(() => {
        if (confirming) {
            keepButton.current?.focus();
        } else if (wasConfirming.current) {
            removeButton.current?.focus();
        }
        wasConfirming.current = confirming;
    }, [confirming]);

    useEffect(() => {
        const onEscape = (event: KeyboardEvent) => {
            if (event.key !== 'Escape' || removing) {
                return;
            }
            if (confirming) {
                setConfirming(false);
            } else {
                onClose();
            }
        };
        window.addEventListener('keydown', onEscape);
        return () => window.removeEventListener('keydown', onEscape);
    }, [confirming, removing, onClose]);

    return (
        <ModalBackdrop onDismiss={onClose} disabled={removing}>
            <section
                ref={dialog}
                className="detail-dialog story-details-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="story-detail-title"
                data-state={confirming ? 'removal-confirmation' : 'story-details'}
            >
                <div className="story-detail-overview">
                    <div
                        className={`cover story-detail-art${hasArtwork ? ' has-artwork' : ''}`}
                        style={coverStyle}
                    >
                        {hasArtwork ? (
                            <img
                                src={artworkURL}
                                alt={t("{0} artwork", story.title)}
                                decoding="async"
                                onError={() => setFailedArtworkURL(artworkURL)}
                            />
                        ) : <b aria-hidden="true">{story.title}</b>}
                    </div>
                    <div className="story-detail-copy">
                        <h3 id="story-detail-title">{story.title}</h3>
                        {(story.official?.publisher || story.author) && (
                            <p className="story-detail-author">
                                {story.official?.publisher || story.author}
                            </p>
                        )}
                        <p className="dialog-description">
                            {description || t("No story description is available yet.")}
                        </p>
                    </div>
                </div>
                <section className="story-detail-warning" aria-label={t('Story warning')}>
                    <div>
                        {story.broken && <strong className="story-warning-label">⚠ {t('Broken')}</strong>}
                        <p>{t('Flag stories that may disrupt the Lunii device menu.')}</p>
                    </div>
                    <BrokenToggle title={story.title} broken={!!story.broken} disabled={flagging || removing} onChange={onFlagChange}/>
                    {flagError && <p role="alert"><OutcomeText text={flagError}/></p>}
                    {flagNotice && <div role="status">{flagNotice}
                        {canUndoFlag && <button type="button" disabled={flagging} onClick={onUndoFlag}>{t('Undo')}</button>}
                    </div>}
                </section>
                <section className="story-detail-tags" aria-label={t("Story tags")}>
                    <div className="story-detail-tags-heading">
                        <h4>{t("Tags")}</h4>
                        <button type="button" disabled={removing} onClick={onEditTags}>
                            {tagSelectionCount > 1 ? t("Edit tags for {0} stories", tagSelectionCount) : t("Edit tags")}
                        </button>
                    </div>
                    <div className="tags">
                        {tags.map((tag) => (
                            <span className="tag" key={tag.key}>
                                <i style={{'--c': tag.color} as CSSProperties} aria-hidden="true"/>
                                {tag.label}
                            </span>
                        ))}
                        {tags.length === 0 && <p>{tagError ? t("Tags could not be loaded.") :
                            tagWorkspace ? t("No tags yet.") : t("Loading tags…")}</p>}
                    </div>
                </section>
                <div className="story-detail-technical">
                    <button
                        type="button"
                        aria-expanded={technicalOpen}
                        aria-controls="story-technical-details"
                        onClick={() => setTechnicalOpen(!technicalOpen)}
                    >
                        {technicalOpen ? t("Hide technical details") : t("Show technical details")}
                    </button>
                    {technicalOpen && (
                        <dl id="story-technical-details" className="dialog-facts">
                            <div><dt>{t("Story UUID")}</dt><dd>{story.uuid}</dd></div>
                            <div>
                                <dt>{t("Archive")}</dt>
                                <dd>{archive?.originalFilename ?? t("Loading…")}</dd>
                            </div>
                            <div><dt>{t("Verification")}</dt><dd>{verification}</dd></div>
                            <div>
                                <dt>{t("Checksum")}</dt>
                                <dd>{archive?.sha256 ?? t("Loading…")}</dd>
                            </div>
                        </dl>
                    )}
                </div>
                {confirming ? (
                    <div className="story-detail-removal">
                        <div className="removal-confirmation" role="alert">
                            <b>{t("Remove from this library?")}</b>
                            <p>
                                “{story.title}{t("” will move to application trash. Your original source file will stay where it is.")}</p>
                        </div>
                        {error && <p className="dialog-error" role="alert">{<OutcomeText text={error}/> }</p>}
                        <div className="dialog-actions">
                            <button
                                ref={keepButton}
                                type="button"
                                disabled={removing}
                                onClick={() => setConfirming(false)}
                            >
                                {t("Keep story")}</button>
                            <button
                                className="danger"
                                type="button"
                                disabled={removing}
                                onClick={onRemove}
                            >
                                {removing ? t("Moving to trash…") : t("Move to trash")}
                            </button>
                        </div>
                    </div>
                ) : (
                    <div className="dialog-actions story-detail-actions">
                        <button
                            ref={removeButton}
                            className="story-remove-action"
                            type="button"
                            onClick={() => setConfirming(true)}
                        >
                            {t("Remove from library…")}</button>
                        <button ref={closeButton} type="button" onClick={onClose}>
                            {t("Close")}</button>
                    </div>
                )}
            </section>
        </ModalBackdrop>
    );
}
