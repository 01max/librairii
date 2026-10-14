import {type CSSProperties, useEffect, useMemo, useRef, useState} from 'react';
import {library} from '../wailsjs/go/models';
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
    onClose: () => void;
    onRemove: () => void;
};

export default function StoryDetailsDialog({
    story, archive, artworkURL, coverStyle, verification,
    removing, error, onClose, onRemove,
}: Props) {
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
        <div className="dialog-backdrop">
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
                                alt={`${story.title} artwork`}
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
                            {description || 'No story description is available yet.'}
                        </p>
                    </div>
                </div>
                <div className="story-detail-technical">
                    <button
                        type="button"
                        aria-expanded={technicalOpen}
                        aria-controls="story-technical-details"
                        onClick={() => setTechnicalOpen(!technicalOpen)}
                    >
                        {technicalOpen ? 'Hide technical details' : 'Show technical details'}
                    </button>
                    {technicalOpen && (
                        <dl id="story-technical-details" className="dialog-facts">
                            <div><dt>Story UUID</dt><dd>{story.uuid}</dd></div>
                            <div>
                                <dt>Archive</dt>
                                <dd>{archive?.originalFilename ?? 'Loading…'}</dd>
                            </div>
                            <div><dt>Verification</dt><dd>{verification}</dd></div>
                            <div>
                                <dt>Checksum</dt>
                                <dd>{archive?.sha256 ?? 'Loading…'}</dd>
                            </div>
                        </dl>
                    )}
                </div>
                {confirming ? (
                    <div className="story-detail-removal">
                        <div className="removal-confirmation" role="alert">
                            <b>Remove from this library?</b>
                            <p>
                                “{story.title}” will move to application trash.
                                Your original source file will stay where it is.
                            </p>
                        </div>
                        {error && <p className="dialog-error" role="alert">{error}</p>}
                        <div className="dialog-actions">
                            <button
                                ref={keepButton}
                                type="button"
                                disabled={removing}
                                onClick={() => setConfirming(false)}
                            >
                                Keep story
                            </button>
                            <button
                                className="danger"
                                type="button"
                                disabled={removing}
                                onClick={onRemove}
                            >
                                {removing ? 'Moving to trash…' : 'Move to trash'}
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
                            Remove from library…
                        </button>
                        <button ref={closeButton} type="button" onClick={onClose}>
                            Close
                        </button>
                    </div>
                )}
            </section>
        </div>
    );
}
