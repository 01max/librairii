import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test, vi} from 'vitest';
import {PreviewShelfSources, ApplyShelfSources} from '../wailsjs/go/main/App';
import {app, shelves} from '../wailsjs/go/models';
import ShelfSources, {AffectedMembers} from './ShelfSources';
vi.mock('../wailsjs/go/main/App', () => ({PreviewShelfSources: vi.fn(), ApplyShelfSources: vi.fn()}));

test('detachment starts with no decisions and commits only explicit Keep all', async () => {
    const plan = new shelves.SourceChangeReview({shelfId: 7, shelfName: 'Weekend', token: 'detach', detached: [{name: 'Favorites'}], affected: [{uuid: 'one', title: 'Story', story: {}, origins: []}], warnings: []});
    vi.mocked(PreviewShelfSources).mockResolvedValue(new app.ShelfSourceReviewResponse({review: plan}));
    vi.mocked(ApplyShelfSources).mockResolvedValue(new app.ShelfResponse({shelf: {id: 7}}));
    const user = userEvent.setup(), changed = vi.fn();
    render(<ShelfSources id={7} sources={[]} collections={[]} onClose={vi.fn()} onChanged={changed}/>);
    await user.click(screen.getByRole('button', {name: 'Preview source changes'}));
    expect(await screen.findByRole('combobox', {name: 'Story'})).toHaveValue('');
    expect(screen.getByRole('button', {name: 'Apply reviewed source changes'})).toBeDisabled();
    expect(ApplyShelfSources).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', {name: 'Keep all and apply sources'}));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(ApplyShelfSources).toHaveBeenCalledWith(7, 'detach', true, []);
});

test('uncertain source coverage disables removal decisions', () => {
    render(<AffectedMembers plan={new shelves.SourceChangeReview({shelfName: 'Weekend', affected: [{uuid: 'one', title: 'Story', story: {}, origins: []}], warnings: [], incomplete: true})} choices={{}} onChange={vi.fn()} busy={false}/>);
    expect(screen.getByRole('option', {name: 'Remove and exclude'})).toBeDisabled();
    expect(screen.getByRole('option', {name: 'Keep'})).toBeEnabled();
});
