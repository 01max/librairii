import {useState} from 'react';
import {library} from '../wailsjs/go/models';

export default function StoryCover({story}: {story: library.StorySummary}) {
    const [failedArtworkID, setFailedArtworkID] = useState<string>();
    const hasArtwork = story.artworkId && failedArtworkID !== story.artworkId;
    return (
        <div className={`cover${hasArtwork ? ' has-artwork' : ''}`}>
            {hasArtwork ? (
                <img
                    src={`/artwork/${encodeURIComponent(story.artworkId!)}`}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    onError={() => setFailedArtworkID(story.artworkId)}
                />
            ) : null}
        </div>
    );
}
