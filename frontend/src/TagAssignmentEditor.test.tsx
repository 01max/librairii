import {render, screen, waitFor} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {
    SetBooleanTag,
    SetChoiceTagValue,
    TagAssignmentWorkspace,
} from '../wailsjs/go/main/App';
import {app, tagging} from '../wailsjs/go/models';
import TagAssignmentEditor from './TagAssignmentEditor';

vi.mock('../wailsjs/go/main/App', () => ({
    SetBooleanTag: vi.fn(),
    SetChoiceTagValue: vi.fn(),
    TagAssignmentWorkspace: vi.fn(),
}));

const setBooleanTag = vi.mocked(SetBooleanTag);
const setChoiceTagValue = vi.mocked(SetChoiceTagValue);
const tagAssignmentWorkspace = vi.mocked(TagAssignmentWorkspace);

const workspace = new tagging.AssignmentWorkspace({
    catalog: {
        definitions: [{
            id: 1,
            key: 'broken',
            label: 'Broken',
            color: '#ff705c',
            kind: 'boolean',
            source: 'builtin',
            protected: true,
            values: [],
        }, {
            id: 2,
            key: 'mood',
            label: 'Mood',
            color: '#405cf5',
            kind: 'choice',
            source: 'user',
            protected: false,
            values: [{
                id: 20,
                definitionId: 2,
                key: 'calm',
                label: 'Calm',
                position: 0,
            }],
        }, {
            id: 3,
            key: 'age',
            label: 'Age',
            color: '#55b79a',
            kind: 'choice',
            source: 'derived',
            protected: true,
            values: [],
        }],
    },
    requestedStories: 2,
    states: [{
        definitionId: 1,
        assignedStories: 1,
        values: [],
    }, {
        definitionId: 2,
        assignedStories: 0,
        values: [{valueId: 20, assignedStories: 2}],
    }, {
        definitionId: 3,
        assignedStories: 0,
        values: [],
    }],
});

beforeEach(() => {
    vi.clearAllMocks();
    tagAssignmentWorkspace.mockResolvedValue(
        new app.TagAssignmentWorkspaceResponse({workspace}),
    );
    setBooleanTag.mockResolvedValue(new app.TagAssignmentResponse({
        result: {
            requestedStories: 2,
            changedStories: 1,
            assignmentsAdded: 1,
            assignmentsRemoved: 0,
        },
    }));
    setChoiceTagValue.mockResolvedValue(new app.TagAssignmentResponse({
        result: {
            requestedStories: 2,
            changedStories: 0,
            assignmentsAdded: 0,
            assignmentsRemoved: 0,
        },
    }));
});

test('shows mixed bulk state and keeps protected derived tags read-only', async () => {
    render(
        <TagAssignmentEditor
            storyIDs={[1, 2]}
            onClose={vi.fn()}
            onWorkspaceChange={vi.fn()}
            onAssignmentsChange={vi.fn()}
        />,
    );

    const broken = await screen.findByRole('checkbox', {name: /Broken warning/});
    expect(broken).toHaveAttribute('aria-checked', 'mixed');
    expect(screen.getByText('Mixed')).toBeInTheDocument();
    expect(screen.getByText(/Derived tags are read-only/)).toBeInTheDocument();
    expect(screen.queryByRole('checkbox', {name: 'Age'})).not.toBeInTheDocument();
});

test('prominently toggles broken across the selected stories', async () => {
    const user = userEvent.setup();
    const onAssignmentsChange = vi.fn();
    render(
        <TagAssignmentEditor
            storyIDs={[1, 2]}
            onClose={vi.fn()}
            onWorkspaceChange={vi.fn()}
            onAssignmentsChange={onAssignmentsChange}
        />,
    );

    await user.click(await screen.findByRole('checkbox', {name: /Broken warning/}));
    expect(setBooleanTag).toHaveBeenCalledWith([1, 2], 1, true);
    expect(await screen.findByRole('status')).toHaveTextContent(
        '1 of 2 selected stories updated.',
    );
    expect(onAssignmentsChange).toHaveBeenCalledTimes(1);
});

test('uses additive choice toggles and reports idempotent operations', async () => {
    const user = userEvent.setup();
    render(
        <TagAssignmentEditor
            storyIDs={[1, 2]}
            onClose={vi.fn()}
            onWorkspaceChange={vi.fn()}
            onAssignmentsChange={vi.fn()}
        />,
    );

    await user.click(await screen.findByRole('checkbox', {name: 'Calm'}));
    expect(setChoiceTagValue).toHaveBeenCalledWith([1, 2], 2, 20, false);
    expect(await screen.findByRole('status')).toHaveTextContent(
        'No stories changed; this tag already matched the selection.',
    );
});

test('closes without a keyboard trap', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
        <TagAssignmentEditor
            storyIDs={[1]}
            onClose={onClose}
            onWorkspaceChange={vi.fn()}
            onAssignmentsChange={vi.fn()}
        />,
    );
    const close = await screen.findByRole('button', {name: 'Close story tags'});
    await waitFor(() => expect(close).toHaveFocus());
    await user.tab({shift: true});
    expect(screen.getByRole('button', {name: 'Done'})).toHaveFocus();
    await user.tab();
    expect(close).toHaveFocus();

    await user.keyboard('{Escape}');
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
});
