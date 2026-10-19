import {useState} from 'react';
import {outcomeKey, outcomeText} from './outcome-message';
import {t} from './i18n';
import {useLanguage} from './use-language';
export default function OutcomeText({text, status}: {text: string; status?: string}) {
    useLanguage();
    const [details, setDetails] = useState(false);
    const summary = outcomeText(text, status);
    const unknown = summary !== text && !outcomeKey(text);
    return <span>{summary}{unknown && <>
        {' '}<button type="button" aria-expanded={details} onClick={() => setDetails(!details)}>{t('Technical details')}</button>
        {details && <span role="note"> {text}</span>}
    </>}</span>;
}
