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
