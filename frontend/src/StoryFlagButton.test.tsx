import {render, screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test, vi} from 'vitest';
import StoryFlagButton from './StoryFlagButton';

test('exposes a labeled keyboard action independently of story selection', async () => {
    const change = vi.fn();
    const user = userEvent.setup();
    render(<StoryFlagButton title="Moon" broken={false} disabled={false} onChange={change}/>);
    await user.tab();
    expect(screen.getByRole('button', {name: 'Mark Moon as broken'})).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(change).toHaveBeenCalledWith(true);
});

test('exposes clear state and disables a pending action', async () => {
    const change = vi.fn();
    const user = userEvent.setup();
    render(<StoryFlagButton title="Moon" broken disabled onChange={change}/>);
    const action = screen.getByRole('button', {name: 'Clear broken flag for Moon'});
    expect(action).toHaveAttribute('aria-pressed', 'true');
    await user.click(action);
    expect(change).not.toHaveBeenCalled();
});
