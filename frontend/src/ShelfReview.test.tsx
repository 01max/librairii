import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test, vi} from 'vitest';
import {PreviewShelfUpdate, ApplyShelfUpdate} from '../wailsjs/go/main/App';
import {app} from '../wailsjs/go/models';
import ShelfReview from './ShelfReview';
vi.mock('../wailsjs/go/main/App', () => ({PreviewShelfUpdate: vi.fn(), ApplyShelfUpdate: vi.fn()}));

test('bounds review rendering and blocks stale decisions until refreshed', async () => {
    vi.mocked(PreviewShelfUpdate).mockResolvedValue(new app.ShelfReviewResponse({review: {token: 'token', additions: Array.from({length: 200}, (_, i) => ({uuid: String(i), title: `Story ${i}`, sources: [{name: 'Favorites'}]})), removals: [], warnings: [], incomplete: true}}));
    vi.mocked(ApplyShelfUpdate).mockResolvedValue(new app.ShelfResponse({error: {code: 'conflict', message: 'Stale review'}}));
    const user = userEvent.setup();
    render(<ShelfReview id={7} onClose={vi.fn()} onApplied={vi.fn()}/>);
    expect(await screen.findByText(/Healthy sources can suggest/)).toBeInTheDocument();
    expect(screen.getAllByRole('combobox')).toHaveLength(25);
    expect(screen.getByRole('button', {name: 'Apply 0 approved changes'})).toBeDisabled();
    await user.selectOptions(screen.getByRole('combobox', {name: 'Decision for Story 0'}), 'add');
    await user.click(screen.getByRole('button', {name: 'Apply 1 approved changes'}));
    await screen.findByText('Stale review');
    expect(ApplyShelfUpdate).toHaveBeenCalledWith(7, 'token', [expect.objectContaining({uuid: '0', action: 'add'})]);
    expect(screen.getByRole('button', {name: 'Apply 1 approved changes'})).toBeDisabled();
    await user.click(screen.getByRole('button', {name: 'Refresh preview'}));
    await waitFor(() => expect(screen.getByRole('button', {name: 'Apply 0 approved changes'})).toBeDisabled());
    await user.click(screen.getByRole('button', {name: 'Approve all additions'}));
    expect(screen.getByRole('button', {name: 'Apply 200 approved changes'})).toBeEnabled();
});
