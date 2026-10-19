import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test, vi} from 'vitest';
import {PreviewShelfCreation, CreateShelfFromReview} from '../wailsjs/go/main/App';
import {app, collections} from '../wailsjs/go/models';
import ShelfCreation from './ShelfCreation';
vi.mock('../wailsjs/go/main/App', () => ({PreviewShelfCreation: vi.fn(), CreateShelfFromReview: vi.fn()}));

test('creation previews any-source union and requires explicit approval before creating', async () => {
    vi.mocked(PreviewShelfCreation).mockResolvedValue(new app.ShelfReviewResponse({review: {token: 'snapshot', additions: [{uuid: 'one', title: 'Flagged story', story: {broken: true}, sources: [{name: 'Favorites'}]}], warnings: []}}));
    vi.mocked(CreateShelfFromReview).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    const onCreated = vi.fn(), user = userEvent.setup();
    render(<ShelfCreation collections={[new collections.Summary({id: 2, name: 'Favorites'})]} manualUUIDs={[]} onClose={vi.fn()} onCreated={onCreated}/>);
    await user.type(screen.getByLabelText('Shelf name'), 'Weekend');
    await user.click(screen.getByLabelText('Favorites'));
    await user.click(screen.getByRole('button', {name: 'Preview initial stories'}));
    expect(PreviewShelfCreation).toHaveBeenCalledWith(expect.objectContaining({name: 'Weekend', collectionIds: [2]}));
    expect(await screen.findByRole('button', {name: 'Create with 0 stories'})).toBeEnabled();
    expect(CreateShelfFromReview).not.toHaveBeenCalled();
    await user.click(screen.getByLabelText('Flagged story · Broken'));
    await user.click(screen.getByRole('button', {name: 'Create with 1 stories'}));
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(7));
    expect(CreateShelfFromReview).toHaveBeenCalledWith('snapshot', ['one']);
});
