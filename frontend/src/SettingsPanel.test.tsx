import {StrictMode} from 'react';
import {act, fireEvent, render, screen, waitFor} from '@testing-library/react';
import {afterEach, beforeEach, expect, test, vi} from 'vitest';
import {Settings, SetUILanguage, SelectLibraryLocation, ConfirmLibrarySetup, DismissSettings} from '../wailsjs/go/main/App';
import {main, app} from '../wailsjs/go/models';
import {EventsOn} from '../wailsjs/runtime/runtime';
import SettingsPanel from './SettingsPanel';
import {setLanguage} from './i18n';
vi.mock('../wailsjs/go/main/App', () => ({Settings: vi.fn(), SetUILanguage: vi.fn(), SelectLibraryLocation: vi.fn(), ConfirmLibrarySetup: vi.fn(), DismissSettings: vi.fn()}));
vi.mock('../wailsjs/runtime/runtime', () => ({EventsOn: vi.fn()}));
const base = (extra: Partial<main.SettingsSnapshot> = {}) => new main.SettingsSnapshot({language:'en',root:'/library',setupRequired:false,isolated:false,pendingOpen:false,...extra});
beforeEach(() => {
 setLanguage('en'); vi.clearAllMocks();
 Object.assign(window, {go:{main:{App:{Settings:() => {}}}}});
 vi.mocked(Settings).mockResolvedValue(base());
 vi.mocked(DismissSettings).mockResolvedValue(undefined);
 vi.mocked(EventsOn).mockReturnValue(() => {});
});
afterEach(() => {setLanguage('en');Reflect.deleteProperty(window,'go');});
test('exports preferences only after explicit settings action and reports success', async () => {
 const exportPreferences=vi.fn().mockResolvedValue(true);
 render(<SettingsPanel onReady={() => {}} onExportPreferences={exportPreferences}/>);
 fireEvent.click(screen.getByRole('button',{name:'Settings'}));
 fireEvent.click(await screen.findByRole('button',{name:'Export preferences…'}));
 expect(await screen.findByRole('status')).toHaveTextContent('Preferences exported.');
 expect(exportPreferences).toHaveBeenCalledOnce();
});
test('native command opens settings and a persisted language updates the UI immediately',async () => {
 render(<SettingsPanel onReady={() => {}}/>);
 await waitFor(() => expect(EventsOn).toHaveBeenCalled());
 act(() => vi.mocked(EventsOn).mock.calls[0][1]());
 const select = await screen.findByRole('combobox',{name:'Interface language'});
 vi.mocked(SetUILanguage).mockResolvedValue(base({language:'fr'}));
 fireEvent.change(select,{target:{value:'fr'}});
 expect(await screen.findByRole('heading',{name:'Réglages'})).toBeInTheDocument();
 expect(SetUILanguage).toHaveBeenCalledWith('fr');
 expect(document.documentElement.lang).toBe('fr');
});
test('setup waits for explicit confirmation and reports a failed attempt',async () => {
 vi.mocked(Settings).mockResolvedValue(base({setupRequired:true}));
 vi.mocked(SelectLibraryLocation).mockResolvedValue(base({setupRequired:true,root:'/autre dossier'}));
 vi.mocked(ConfirmLibrarySetup).mockResolvedValue(new app.MutationResponse({error:{code:'invalid_input',message:'settings.destination_invalid'}}));
 const ready=vi.fn();render(<SettingsPanel onReady={ready}/>);
 const confirm=await screen.findByRole('button',{name:'Use this folder'});
 expect(ConfirmLibrarySetup).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'Change location…'}));
 await screen.findByText('/autre dossier');
 fireEvent.click(confirm);
 await screen.findByRole('alert');expect(ready).not.toHaveBeenCalled();
 vi.mocked(ConfirmLibrarySetup).mockResolvedValue(new app.MutationResponse({success:true}));
 vi.mocked(Settings).mockResolvedValue(base());
 fireEvent.click(confirm);await waitFor(() => expect(ready).toHaveBeenCalledOnce());
});


test('retains an early native open request through StrictMode until explicitly dismissed',async () => {
 vi.mocked(Settings).mockResolvedValue(base({pendingOpen:true}));
 render(<StrictMode><SettingsPanel onReady={() => {}}/></StrictMode>);
 await screen.findByRole('dialog');
 fireEvent.click(screen.getByRole('button',{name:/^Close$/}));
 await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
 expect(DismissSettings).toHaveBeenCalledOnce();
});
