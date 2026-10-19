import {render, screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect, test} from 'vitest';
import {shelves} from '../wailsjs/go/models';
import ReviewWarnings from './ReviewWarnings';

test('keeps large unavailable warning sets paginated and discoverable', async () => {
    const user = userEvent.setup();
    const {container} = render(<ReviewWarnings warnings={Array.from({length: 1000}, (_, index) => new shelves.ReviewWarning({storyUuid: `uuid-${index}`, code: 'story_unavailable', message: `Warning ${index}`}))}/>);
    expect(container.querySelectorAll('li')).toHaveLength(25);
    await user.click(screen.getByText('Review warnings (1,000)'));
    expect(screen.getByText('Warning 0')).toBeVisible();
    expect(screen.queryByText('Warning 25')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Next →'}));
    expect(screen.getByText('Warning 25')).toBeVisible();
    expect(screen.queryByText('Warning 0')).not.toBeInTheDocument();
    expect(container.querySelectorAll('li')).toHaveLength(25);
});
