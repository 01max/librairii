import {useSyncExternalStore} from 'react';
import {currentLanguage, subscribeLanguage} from './i18n';
export function useLanguage() { return useSyncExternalStore(subscribeLanguage, currentLanguage); }
