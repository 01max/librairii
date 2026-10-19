import {afterEach, expect, test} from 'vitest';
import {render, screen} from '@testing-library/react';
import {operations} from '../wailsjs/go/models';
import {setLanguage} from './i18n';
import ExportReport from './ExportReport';

afterEach(() => setLanguage('en'));

test('French export reports retain successes for current and unknown outcomes', () => {
    setLanguage('fr');
    const operation = operations.Snapshot.createFrom({
        id: 'export', status: 'succeeded', kind: 'export', items: [
            {id: 'known', storyTitle: 'Known story', status: 'succeeded', outcomeMessage: 'Story archive exported.'},
            {id: 'future', storyTitle: 'Future story', status: 'succeeded', outcomeMessage: 'Future successful export'},
        ],
    });
    render(<ExportReport operation={operation} onReveal={() => {}}/>);
    expect(screen.getByText('Archive de l’histoire exportée.')).toBeInTheDocument();
    expect(screen.getByText('Terminé')).toBeInTheDocument();
    expect(screen.queryByText('L’action n’a pas pu être effectuée')).not.toBeInTheDocument();
});
