import {render, screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test, vi} from 'vitest';
import BrokenToggle from './BrokenToggle';

test('exposes a labeled keyboard action independently of story selection', async () => {
    const change = vi.fn();
    const user = userEvent.setup();
    render(<BrokenToggle title="Moon" broken={false} disabled={false} onChange={change}/>);
    await user.tab();
    expect(screen.getByRole('switch', {name: 'Broken: Moon'})).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(change).toHaveBeenCalledWith(true);
});

test('exposes clear state and disables a pending action', async () => {
    const change = vi.fn();
    const user = userEvent.setup();
    render(<BrokenToggle title="Moon" broken disabled onChange={change}/>);
    const action = screen.getByRole('switch', {name: 'Broken: Moon'});
    expect(action).toHaveAttribute('aria-checked', 'true');
    await user.click(action);
    expect(change).not.toHaveBeenCalled();
});
