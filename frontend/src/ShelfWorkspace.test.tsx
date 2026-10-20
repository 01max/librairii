import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {OpenShelf, SetShelfStoryProtected, ClearShelfExclusion, SetStoriesBroken, PreviewShelfUpdate, ApplyShelfUpdate, PreviewShelfSources, ApplyShelfSources, RemoveStoriesFromShelf, ReorderShelfStories} from '../wailsjs/go/main/App';
import {app, library} from '../wailsjs/go/models';
import ShelfWorkspace from './ShelfWorkspace';
import {DEFAULT_COLLECTION_QUERY} from './query-codec';
vi.mock('../wailsjs/go/main/App', () => ({OpenShelf: vi.fn(), SetShelfStoryProtected: vi.fn(), ClearShelfExclusion: vi.fn(), SetStoriesBroken: vi.fn(), RestoreStoryFlags: vi.fn(), PreviewShelfUpdate: vi.fn(), ApplyShelfUpdate: vi.fn(), PreviewShelfSources: vi.fn(), ApplyShelfSources: vi.fn(), RemoveStoriesFromShelf: vi.fn(), ReorderShelfStories: vi.fn()}));
let broken = false;
beforeEach(() => {
    broken = false; vi.clearAllMocks();
    vi.mocked(OpenShelf).mockImplementation(async () => new app.ShelfEvaluationResponse({evaluation: {shelf: {id: 7, name: 'Weekend', count: 2}, sources: [{id: 1, active: true, name: 'Favorites'}], members: [
        {uuid: 'one', cachedTitle: 'First', position: 0, protected: false, story: {id: 1, uuid: 'one', title: 'First', broken}, origins: []},
        {uuid: 'missing', cachedTitle: 'Lost', position: 1, protected: true, origins: []},
    ], exclusions: [{uuid: 'excluded', title: 'Excluded story'}], page: 1, pageSize: 25, totalItems: 2, totalPages: 1}}));
    vi.mocked(SetShelfStoryProtected).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    vi.mocked(ClearShelfExclusion).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    vi.mocked(SetStoriesBroken).mockImplementation(async () => {broken = true; return new app.StoryFlagsResponse({result: new library.FlagResult({requestedStories: 1, changedStories: 1, changes: [{storyId: 1, previous: false, current: true}]})});});
});
const mount = (onExport = vi.fn()) => render(<ShelfWorkspace id={7} collections={[]} revision={0} query={{...DEFAULT_COLLECTION_QUERY, name: 'view only'}} onQuery={vi.fn()} onChanged={vi.fn()} onDeleted={vi.fn()} onInspect={vi.fn()} onExport={onExport}/>);

test('exports distinguish whole Shelf refinements and selected unavailable UUIDs', async () => {
    const onExport = vi.fn(), user = userEvent.setup(); mount(onExport);
    await screen.findByRole('heading', {name: 'Weekend'});
    await user.click(screen.getByRole('button', {name: 'Export whole Shelf'}));
    expect(onExport).toHaveBeenLastCalledWith(expect.objectContaining({sourceType: 'curated_shelf', shelfIds: [7]}));
    await user.click(screen.getByRole('button', {name: 'Export current result'}));
    expect(onExport).toHaveBeenLastCalledWith(expect.objectContaining({sourceType: 'current_query', queryShelfId: 7, query: expect.objectContaining({name: 'view only'})}));
    await user.click(screen.getByRole('checkbox', {name: 'Lost'}));
    await user.click(screen.getByRole('button', {name: 'Export selection'}));
    expect(onExport).toHaveBeenLastCalledWith(expect.objectContaining({sourceType: 'selection', queryShelfId: 7, shelfSelectionUuids: ['missing']}));
});

test('flag changes retain membership selection and direct control focus', async () => {
    const user = userEvent.setup(); mount();
    await user.click(await screen.findByRole('checkbox', {name: 'First'}));
    await user.click(screen.getByRole('switch', {name: 'Broken: Selected stories'}));
    expect(await screen.findByRole('switch', {name: 'Broken: Selected stories'})).toHaveFocus();
    expect(screen.getByRole('checkbox', {name: 'First'})).toBeChecked();
    expect(screen.getByText('Broken · excluded from exports')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', {name: 'Lost'})).toBeInTheDocument();
});

test('protection and exclusions change only through explicit controls', async () => {
    const user = userEvent.setup(); mount();
    await user.click(await screen.findByRole('button', {name: 'Protect First'}));
    expect(SetShelfStoryProtected).toHaveBeenCalledWith(7, 'one', true);
    await user.click(screen.getByRole('button', {name: 'Manage exclusions (1)'}));
    expect(ClearShelfExclusion).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', {name: 'Allow suggestions for Excluded story'}));
    await waitFor(() => expect(ClearShelfExclusion).toHaveBeenCalledWith(7, 'excluded'));
});

test.each(['review', 'sources'])('%s removals discard removed selections and retain valid selections across pages', async (flow) => {
    let removed = false;
    vi.mocked(OpenShelf).mockImplementation(async (_id, query) => new app.ShelfEvaluationResponse({evaluation: {
        shelf: {id: 7, name: 'Weekend', count: removed ? 1 : 2}, sources: [{id: 1, collectionId: 1, active: true, name: 'Favorites'}],
        members: removed || query.page === 2
            ? [{uuid: 'kept', cachedTitle: 'Kept', story: {id: 2, uuid: 'kept', title: 'Kept'}}]
            : [{uuid: 'one', cachedTitle: 'First', story: {id: 1, uuid: 'one', title: 'First'}}],
        exclusions: [], page: removed ? 1 : query.page, pageSize: 1, totalItems: removed ? 1 : 2, totalPages: removed ? 1 : 2,
    }}));
    vi.mocked(PreviewShelfUpdate).mockResolvedValue(new app.ShelfReviewResponse({review: {token: 'review', additions: [], removals: [{uuid: 'one', title: 'First'}], warnings: []}}));
    vi.mocked(PreviewShelfSources).mockResolvedValue(new app.ShelfSourceReviewResponse({review: {token: 'sources', shelfName: 'Weekend', detached: [{name: 'Favorites'}], affected: [{uuid: 'one', title: 'First', story: {id: 1}}], warnings: []}}));
    const apply = async () => {removed = true; return new app.ShelfResponse({shelf: {id: 7}});};
    vi.mocked(ApplyShelfUpdate).mockImplementation(apply);
    vi.mocked(ApplyShelfSources).mockImplementation(apply);
    vi.mocked(RemoveStoriesFromShelf).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    const user = userEvent.setup(), onExport = vi.fn();
    const props = {id: 7, collections: [], revision: 0, onQuery: vi.fn(), onChanged: vi.fn(), onDeleted: vi.fn(), onInspect: vi.fn(), onExport};
    const {rerender} = render(<ShelfWorkspace {...props} query={{...DEFAULT_COLLECTION_QUERY, pageSize: 1}}/>);
    await user.click(await screen.findByRole('checkbox', {name: 'First'}));
    rerender(<ShelfWorkspace {...props} query={{...DEFAULT_COLLECTION_QUERY, pageSize: 1, page: 2}}/>);
    await user.click(await screen.findByRole('checkbox', {name: 'Kept'}));
    if (flow === 'review') {
        await user.click(screen.getByRole('button', {name: 'Review updates'}));
        await user.click(await screen.findByRole('button', {name: 'Approve all removals'}));
        await user.click(screen.getByRole('button', {name: 'Apply 1 approved changes'}));
    } else {
        await user.click(screen.getByRole('button', {name: 'Manage sources'}));
        await user.click(screen.getByRole('button', {name: 'Preview source changes'}));
        await user.selectOptions(await screen.findByRole('combobox', {name: 'First'}), 'remove');
        await user.click(screen.getByRole('button', {name: 'Apply reviewed source changes'}));
    }
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByRole('checkbox', {name: 'Kept'})).toBeChecked();
    await user.click(screen.getByRole('button', {name: 'Export selection'}));
    expect(onExport).toHaveBeenLastCalledWith(expect.objectContaining({shelfSelectionUuids: ['kept']}));
    await user.click(screen.getByRole('button', {name: 'Remove selected and exclude from suggestions'}));
    expect(RemoveStoriesFromShelf).toHaveBeenCalledWith(7, ['kept']);
});

test.each([2, 1])('navigates from the returned page after membership shrinks to %i pages', async (remainingPages) => {
    let shrunk = false;
    vi.mocked(OpenShelf).mockImplementation(async (_id, query) => new app.ShelfEvaluationResponse({evaluation: {
        shelf: {id: 7, name: 'Weekend', count: shrunk ? remainingPages : 4}, sources: [],
        members: [{uuid: 'one', cachedTitle: 'First', story: {id: 1, uuid: 'one', title: 'First'}}], exclusions: [],
        page: shrunk ? Math.min(query.page, remainingPages) : query.page, pageSize: 1, totalItems: shrunk ? remainingPages : 4, totalPages: shrunk ? remainingPages : 4,
    }}));
    vi.mocked(PreviewShelfUpdate).mockResolvedValue(new app.ShelfReviewResponse({review: {token: 'review', additions: [], removals: [{uuid: 'removed', title: 'Removed'}], warnings: []}}));
    vi.mocked(ApplyShelfUpdate).mockImplementation(async () => {shrunk = true; return new app.ShelfResponse({shelf: {id: 7}});});
    const onQuery = vi.fn(), user = userEvent.setup();
    render(<ShelfWorkspace id={7} collections={[]} revision={0} query={{...DEFAULT_COLLECTION_QUERY, page: 4, pageSize: 1}} onQuery={onQuery} onChanged={vi.fn()} onDeleted={vi.fn()} onInspect={vi.fn()} onExport={vi.fn()}/>);
    await screen.findByText('Page 4 of 4');
    await user.click(screen.getByRole('button', {name: 'Review updates'}));
    await user.click(await screen.findByRole('button', {name: 'Approve all removals'}));
    await user.click(screen.getByRole('button', {name: 'Apply 1 approved changes'}));
    await screen.findByText(`Page ${remainingPages} of ${remainingPages}`);
    expect(screen.getByRole('button', {name: 'Next →'})).toBeDisabled();
    const previous = screen.getByRole('button', {name: '← Previous'});
    if (remainingPages === 1) {
        expect(previous).toBeDisabled();
    } else {
        await user.click(previous);
        expect(onQuery).toHaveBeenCalledWith(expect.objectContaining({page: 1}));
    }
});

test('reorders the full Shelf using valid pages even when it exceeds 100 members', async () => {
    const members = Array.from({length: 101}, (_, i) => ({uuid: String(i), cachedTitle: `Story ${i}`, position: i, protected: true}));
    vi.mocked(OpenShelf).mockImplementation(async (_id, query) => {
        if (query.pageSize > 100) return new app.ShelfEvaluationResponse({error: {message: 'Invalid page size'}});
        return new app.ShelfEvaluationResponse({evaluation: {
            shelf: {id: 7, name: 'Weekend', count: members.length}, sources: [], exclusions: [],
            members: members.slice((query.page - 1) * query.pageSize, query.page * query.pageSize),
            page: query.page, pageSize: query.pageSize, totalItems: members.length, totalPages: Math.ceil(members.length / query.pageSize),
        }});
    });
    vi.mocked(ReorderShelfStories).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    const user = userEvent.setup();
    render(<ShelfWorkspace id={7} collections={[]} revision={0} query={DEFAULT_COLLECTION_QUERY} onQuery={vi.fn()} onChanged={vi.fn()} onDeleted={vi.fn()} onInspect={vi.fn()} onExport={vi.fn()}/>);
    await user.click(await screen.findByRole('button', {name: 'Move Story 0 down'}));
    await waitFor(() => expect(ReorderShelfStories).toHaveBeenCalledWith(7, ['1', '0', ...members.slice(2).map((member) => member.uuid)]));
    expect(OpenShelf).toHaveBeenCalledWith(7, expect.objectContaining({page: 2, pageSize: 100, sort: ''}));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
