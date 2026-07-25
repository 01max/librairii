import {type CSSProperties, useEffect, useMemo, useState} from 'react';
import './App.css';
import {
    ApplicationStatus,
    ListStories,
    StoryDetail as LoadStoryDetail,
} from '../wailsjs/go/main/App';
import {library} from '../wailsjs/go/models';

const coverPalettes = [
    ['#31559f', '#f7c85b', '#e06a53'],
    ['#ef765f', '#ffe380', '#7354b8'],
    ['#48a88d', '#f5bc41', '#226a63'],
    ['#6a66d9', '#fc8e73', '#343272'],
    ['#c06d9d', '#f8d164', '#743d7b'],
    ['#de9954', '#fff0ab', '#887b42'],
    ['#4aa9c9', '#ffe079', '#326779'],
    ['#d77c6f', '#ffd86b', '#8c4d4c'],
] as const;

type CoverStyle = CSSProperties & {
    '--sky': string;
    '--sun': string;
    '--land': string;
};

function paletteFor(storyID: number): CoverStyle {
    const palette = coverPalettes[Math.abs(storyID) % coverPalettes.length];
    return {
        '--sky': palette[0],
        '--sun': palette[1],
        '--land': palette[2],
    };
}

function chunkStories(stories: library.StorySummary[]): library.StorySummary[][] {
    const rows: library.StorySummary[][] = [];
    for (let index = 0; index < stories.length; index += 6) {
        rows.push(stories.slice(index, index + 6));
    }
    return rows;
}

function formatBytes(byteSize: number): string {
    if (byteSize < 1024 * 1024) {
        return `${Math.max(1, Math.round(byteSize / 1024))} KB`;
    }
    return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}

function compatibilityLabel(value: string): string {
    switch (value) {
        case 'compatible':
            return 'Verified';
        case 'missing':
            return 'Missing';
        default:
            return 'Needs attention';
    }
}

function App() {
    const [applicationState, setApplicationState] = useState('initializing');
    const [page, setPage] = useState<library.Page | null>(null);
    const [selectedID, setSelectedID] = useState<number | null>(null);
    const [detail, setDetail] = useState<library.StoryDetail | null>(null);

    useEffect(() => {
        let active = true;
        void ApplicationStatus().then(async (response) => {
            const state = response.error?.code ?? response.status.state;
            if (!active) {
                return;
            }
            setApplicationState(state);
            if (state !== 'ready') {
                return;
            }
            const result = await ListStories({
                page: 1,
                pageSize: 12,
                sort: 'imported_desc',
            });
            if (!active || !result.page) {
                return;
            }
            setPage(result.page);
            setDetail(null);
            setSelectedID(result.page.stories[0]?.id ?? null);
        });
        return () => {
            active = false;
        };
    }, []);

    useEffect(() => {
        if (selectedID === null) {
            return;
        }
        let active = true;
        void LoadStoryDetail(selectedID).then((response) => {
            if (active) {
                setDetail(response.detail ?? null);
            }
        });
        return () => {
            active = false;
        };
    }, [selectedID]);

    const stories = useMemo(() => page?.stories ?? [], [page]);
    const rows = useMemo(() => chunkStories(stories), [stories]);
    const selectedSummary = stories.find((story) => story.id === selectedID) ?? null;
    const selected = detail?.story ?? selectedSummary;
    const selectedPalette = selected ? paletteFor(selected.id) : undefined;

    return (
        <div className="app">
            <span className="sr-only" data-testid="application-state">
                Application state: {applicationState}
            </span>
            <aside className="rail" aria-label="Primary navigation">
                <div className="mark" aria-label="Librairii">
                    <i/><i/><i/>
                </div>
                <nav aria-label="Primary navigation">
                    <button className="active" type="button" aria-label="Story collection">▦</button>
                    <button type="button" aria-label="Saved shelves">◇</button>
                    <button type="button" aria-label="Imports">↓</button>
                    <button type="button" aria-label="Exports">↗</button>
                </nav>
                <div className="avatar" aria-label="Local profile">ML</div>
            </aside>

            <aside className="filters" aria-label="Saved shelves and refinements">
                <h1>Librairii</h1>
                <div className="path">Local story archive</div>
                <div className="caption">Saved shelves</div>
                <nav className="saved">
                    <button className="active" type="button">
                        <i style={{'--c': '#405cf5'} as CSSProperties}/>
                        All stories
                        <span>{page?.totalItems ?? 0}</span>
                    </button>
                </nav>
                <div className="caption">Refine this shelf</div>
                <div className="facet">
                    <div className="facet-title">Age <b>−</b></div>
                    <label className="choice">
                        <input type="checkbox" disabled/>
                        <i style={{'--c': '#ff705c'} as CSSProperties}/>
                        3–5 years
                        <span>0</span>
                    </label>
                    <label className="choice">
                        <input type="checkbox" disabled/>
                        <i style={{'--c': '#ff705c'} as CSSProperties}/>
                        6–8 years
                        <span>0</span>
                    </label>
                </div>
                <div className="facet">
                    <div className="facet-title">Language <b>＋</b></div>
                </div>
                <div className="facet">
                    <div className="facet-title">Import status <b>＋</b></div>
                </div>
                <button className="manage" type="button">＋ Manage tags</button>
            </aside>

            <main className="main">
                <div className="top">
                    <label className="search">
                        <span aria-hidden="true">⌕</span>
                        <input
                            type="search"
                            aria-label="Search stories"
                            placeholder={`Search ${page?.totalItems ?? 0} stories, authors, publishers…`}
                        />
                    </label>
                    <button type="button">↻ Sync</button>
                    <button className="import" type="button">＋ Import stories</button>
                </div>

                <div className="heading">
                    <div>
                        <h2>My story shelves</h2>
                        <p>Stories in your local archive · {page?.totalItems ?? 0} archives</p>
                    </div>
                    <div className="sort">Recently added ⌄</div>
                </div>

                {rows.map((row, rowIndex) => (
                    <section className="shelf" key={rowIndex}>
                        <div className="shelf-head">
                            <h3>{rowIndex === 0 ? 'Recently added' : 'More stories'}</h3>
                            <span>{row.length} {row.length === 1 ? 'story' : 'stories'} · Local archive</span>
                            <button type="button">View all →</button>
                        </div>
                        <div className="story-row">
                            {row.map((story) => (
                                <button
                                    className={`story${story.id === selectedID ? ' selected' : ''}`}
                                    style={paletteFor(story.id)}
                                    type="button"
                                    key={story.id}
                                    aria-pressed={story.id === selectedID}
                                    onClick={() => {
                                        setDetail(null);
                                        setSelectedID(story.id);
                                    }}
                                >
                                    <div className="cover"><b>{story.title}</b></div>
                                    <small>
                                        {story.author || story.uuid}
                                    </small>
                                </button>
                            ))}
                        </div>
                    </section>
                ))}
            </main>

            {selected && (
                <aside className="drawer" aria-label={`${selected.title} details`}>
                    <div className="drawer-art" style={selectedPalette}/>
                    <div className="drawer-info">
                        <h3>{selected.title}</h3>
                        <p>
                            {selected.author || 'Local story'} · {selected.sources.title} metadata
                        </p>
                        <div className="facts">
                            <div className="fact">
                                <span>Format</span>
                                <b>{selected.detectedFormat}</b>
                            </div>
                            <div className="fact">
                                <span>Size</span>
                                <b>{formatBytes(selected.byteSize)}</b>
                            </div>
                            <div className="fact">
                                <span>Status</span>
                                <b>{compatibilityLabel(selected.compatibility)}</b>
                            </div>
                        </div>
                    </div>
                    <div className="drawer-tags">
                        <div className="tag-title">
                            My tags
                            <button type="button">Edit tags</button>
                        </div>
                        <div className="tags"/>
                        <div className="archive">
                            {detail?.archive.originalFilename ?? 'Loading archive details…'}
                            {' · '}
                            {formatBytes(detail?.archive.byteSize ?? selected.byteSize)}
                            {' · '}
                            {compatibilityLabel(
                                detail?.archive.verification ?? selected.compatibility,
                            )}
                        </div>
                    </div>
                    <div className="drawer-actions">
                        <button type="button">Open details</button>
                        <button className="export" type="button">Add to export →</button>
                    </div>
                </aside>
            )}
        </div>
    );
}

export default App;
