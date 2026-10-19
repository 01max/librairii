import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {OpenShelf, SetShelfStoryProtected, ClearShelfExclusion, SetStoriesBroken} from '../wailsjs/go/main/App';
import {app, library} from '../wailsjs/go/models';
import ShelfWorkspace from './ShelfWorkspace';
import {DEFAULT_COLLECTION_QUERY} from './query-codec';
vi.mock('../wailsjs/go/main/App', () => ({OpenShelf: vi.fn(), SetShelfStoryProtected: vi.fn(), ClearShelfExclusion: vi.fn(), SetStoriesBroken: vi.fn(), RestoreStoryFlags: vi.fn()}));
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
