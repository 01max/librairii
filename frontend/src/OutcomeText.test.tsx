import {afterEach, expect, test} from 'vitest';
import {fireEvent, render, screen} from '@testing-library/react';
import {setLanguage} from './i18n';
import OutcomeText from './OutcomeText';
afterEach(() => setLanguage('en'));
test('unknown historical details have a French summary with preserved diagnostics', () => {
 setLanguage('fr');render(<OutcomeText text="Legacy database diagnostic"/>);
 expect(screen.getByText('L’action n’a pas pu être effectuée')).toBeInTheDocument();
 expect(screen.getByText('Détails techniques')).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Détails techniques'}));
 expect(screen.getByText('Legacy database diagnostic')).toBeInTheDocument();
});

test('unknown successful outcomes keep their success summary and original details', () => {
 setLanguage('fr');
 render(<OutcomeText text="Future successful outcome" status="succeeded"/>);
 expect(screen.getByText('Terminé')).toBeInTheDocument();
 expect(screen.queryByText('L’action n’a pas pu être effectuée')).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole('button', {name: 'Détails techniques'}));
 expect(screen.getByText('Future successful outcome')).toBeInTheDocument();
});

test('successful current outcomes translate without a diagnostic disclosure', () => {
 setLanguage('fr');
 render(<OutcomeText text="Story imported." status="succeeded"/>);
 expect(screen.getByText('Histoire importée.')).toBeInTheDocument();
 expect(screen.queryByRole('button', {name: 'Détails techniques'})).not.toBeInTheDocument();
});
