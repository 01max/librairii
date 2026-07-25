import {fireEvent, render, screen, waitFor, within} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {beforeEach, expect, test, vi} from 'vitest';
import {
    CreateTagDefinition,
    CreateTagValue,
    DeleteTagDefinition,
    DeleteTagValue,
    PlanTagDefinitionDeletion,
    PlanTagValueDeletion,
    RecolorTagDefinition,
    RenameTagDefinition,
    RenameTagValue,
    ReorderTagDefinitions,
    ReorderTagValues,
    TagCatalog,
} from '../wailsjs/go/main/App';
import {app, tagging} from '../wailsjs/go/models';
import TagManager from './TagManager';

vi.mock('../wailsjs/go/main/App', () => ({
    CreateTagDefinition: vi.fn(),
    CreateTagValue: vi.fn(),
    DeleteTagDefinition: vi.fn(),
    DeleteTagValue: vi.fn(),
    PlanTagDefinitionDeletion: vi.fn(),
    PlanTagValueDeletion: vi.fn(),
    RecolorTagDefinition: vi.fn(),
    RenameTagDefinition: vi.fn(),
    RenameTagValue: vi.fn(),
    ReorderTagDefinitions: vi.fn(),
    ReorderTagValues: vi.fn(),
    TagCatalog: vi.fn(),
}));

const tagCatalog = vi.mocked(TagCatalog);
const createTagDefinition = vi.mocked(CreateTagDefinition);
const createTagValue = vi.mocked(CreateTagValue);
const deleteTagDefinition = vi.mocked(DeleteTagDefinition);
const deleteTagValue = vi.mocked(DeleteTagValue);
const planTagDefinitionDeletion = vi.mocked(PlanTagDefinitionDeletion);
const planTagValueDeletion = vi.mocked(PlanTagValueDeletion);
const recolorTagDefinition = vi.mocked(RecolorTagDefinition);
const renameTagDefinition = vi.mocked(RenameTagDefinition);
const renameTagValue = vi.mocked(RenameTagValue);
const reorderTagDefinitions = vi.mocked(ReorderTagDefinitions);
const reorderTagValues = vi.mocked(ReorderTagValues);

const catalog = new tagging.Catalog({
    definitions: [{
        id: 1,
        key: 'broken',
        normalizedKey: 'broken',
        label: 'Broken',
        color: '#ff705c',
        kind: 'boolean',
        source: 'builtin',
        presentation: 'warning',
        position: 0,
        protected: true,
        values: [],
    }, {
        id: 2,
        key: 'mood',
        normalizedKey: 'mood',
        label: 'Mood',
        color: '#405cf5',
        kind: 'choice',
        source: 'user',
        presentation: 'default',
        position: 0,
        protected: false,
        values: [{
            id: 20,
            definitionId: 2,
            key: 'calm',
            normalizedKey: 'calm',
            label: 'Calm',
            position: 0,
        }, {
            id: 21,
            definitionId: 2,
            key: 'bold',
            normalizedKey: 'bold',
            label: 'Bold',
            position: 1,
        }],
    }, {
        id: 3,
        key: 'favorite',
        normalizedKey: 'favorite',
        label: 'Favorite',
        color: '#55b79a',
        kind: 'boolean',
        source: 'user',
        presentation: 'default',
        position: 1,
        protected: false,
        values: [],
    }],
});

beforeEach(() => {
    vi.clearAllMocks();
    tagCatalog.mockResolvedValue(new app.TagCatalogResponse({catalog}));
    createTagDefinition.mockResolvedValue(new app.TagDefinitionResponse({}));
    createTagValue.mockResolvedValue(new app.TagValueResponse({}));
    deleteTagDefinition.mockResolvedValue(new app.MutationResponse({success: true}));
    deleteTagValue.mockResolvedValue(new app.MutationResponse({success: true}));
    recolorTagDefinition.mockResolvedValue(new app.TagDefinitionResponse({}));
    renameTagDefinition.mockResolvedValue(new app.TagDefinitionResponse({}));
    renameTagValue.mockResolvedValue(new app.TagValueResponse({}));
    reorderTagDefinitions.mockResolvedValue(new app.MutationResponse({success: true}));
    reorderTagValues.mockResolvedValue(new app.MutationResponse({success: true}));
});

test('presents protected and custom definitions with non-color-only state labels', async () => {
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);

    expect(await screen.findByText('Built-in · Boolean · Protected')).toBeInTheDocument();
    const broken = screen.getByText('Broken').closest('article');
    expect(broken).not.toBeNull();
    expect(within(broken!).queryByRole('button', {name: 'Delete Broken'}))
        .not.toBeInTheDocument();
    expect(screen.getByText('Custom · Choice')).toBeInTheDocument();
});

test('submits typed definitions and renders backend validation inline', async () => {
    const user = userEvent.setup();
    createTagDefinition.mockResolvedValue(new app.TagDefinitionResponse({
        error: {code: 'invalid_input', message: 'Key must use lowercase words.'},
    }));
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);
    await screen.findByText('Broken');

    const form = screen.getByRole('heading', {name: 'New tag'}).closest('form');
    await user.type(within(form!).getByLabelText('Key'), 'Bad Key');
    await user.type(within(form!).getByLabelText('Label'), 'Reading level');
    await user.selectOptions(within(form!).getByLabelText('State'), 'choice');
    await user.click(within(form!).getByRole('button', {name: 'Add tag'}));

    expect(createTagDefinition).toHaveBeenCalledWith(expect.objectContaining({
        key: 'Bad Key',
        label: 'Reading level',
        kind: 'choice',
    }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
        'Key must use lowercase words.',
    );
});

test('reorders definitions and choice values with explicit keyboard buttons', async () => {
    const user = userEvent.setup();
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);

    await user.click(await screen.findByRole('button', {name: 'Move Favorite up'}));
    expect(reorderTagDefinitions).toHaveBeenCalledWith([3, 2]);

    await user.click(screen.getByRole('button', {name: 'Move Bold up'}));
    expect(reorderTagValues).toHaveBeenCalledWith(2, [21, 20]);
});

test('renames and recolors a custom definition without changing its key', async () => {
    const user = userEvent.setup();
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);
    const mood = (await screen.findByText('Mood')).closest('article');
    const label = within(mood!).getByDisplayValue('Mood');
    const color = within(mood!).getByLabelText('Color');

    await user.clear(label);
    await user.type(label, 'Emotions');
    fireEvent.change(color, {target: {value: '#263a8b'}});
    await user.click(within(label.closest('form')!).getByRole('button', {name: 'Save'}));

    expect(renameTagDefinition).toHaveBeenCalledWith(2, 'Emotions');
    expect(recolorTagDefinition).toHaveBeenCalledWith(2, '#263a8b');
});

test('plans destructive changes and confirms the displayed assignment impact', async () => {
    const user = userEvent.setup();
    const plan = new tagging.DefinitionDeletionPlan({
        definition: catalog.definitions[1],
        valueCount: 2,
        assignmentCount: 12,
        affectedShelfCount: 3,
    });
    planTagDefinitionDeletion.mockResolvedValue(
        new app.TagDefinitionDeletionPlanResponse({plan}),
    );
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);
    const mood = (await screen.findByText('Mood')).closest('article');

    await user.click(within(mood!).getByRole('button', {name: 'Delete Mood'}));
    expect(await screen.findByText(
        '12 assignments, 2 values, and 3 saved shelves are affected.',
    )).toBeInTheDocument();
    await user.click(screen.getByRole('button', {name: 'Delete permanently'}));

    expect(deleteTagDefinition).toHaveBeenCalledWith(plan);
});

test('renames and deletes choice values and closes from the keyboard', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const plan = new tagging.ValueDeletionPlan({
        value: catalog.definitions[1].values[0],
        assignmentCount: 4,
        affectedShelfCount: 1,
    });
    planTagValueDeletion.mockResolvedValue(new app.TagValueDeletionPlanResponse({plan}));
    render(<TagManager onClose={onClose} onCatalogChange={vi.fn()}/>);

    const label = await screen.findByLabelText('Label for calm');
    await user.clear(label);
    await user.type(label, 'Peaceful');
    await user.click(within(label.closest('form')!).getByRole('button', {name: 'Save'}));
    expect(renameTagValue).toHaveBeenCalledWith(20, 'Peaceful');

    await user.click(within(label.closest('form')!).getByRole('button', {name: 'Delete Calm'}));
    await user.click(await screen.findByRole('button', {name: 'Delete permanently'}));
    expect(deleteTagValue).toHaveBeenCalledWith(plan);

    await user.keyboard('{Escape}');
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
});

test('creates a choice value from the inline value form', async () => {
    const user = userEvent.setup();
    render(<TagManager onClose={vi.fn()} onCatalogChange={vi.fn()}/>);

    const key = await screen.findByLabelText('Mood value key');
    await user.type(key, 'dreamy');
    await user.type(screen.getByLabelText('Mood value label'), 'Dreamy');
    await user.click(within(key.closest('form')!).getByRole('button', {name: 'Add value'}));

    expect(createTagValue).toHaveBeenCalledWith(expect.objectContaining({
        definitionId: 2,
        key: 'dreamy',
        label: 'Dreamy',
    }));
});
