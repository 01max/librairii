import {useState} from 'react';
import {shelves} from '../wailsjs/go/models';
import OutcomeText from './OutcomeText';
import {t} from './i18n';

export default function ReviewWarnings({warnings}: {warnings: shelves.ReviewWarning[]}) {
    const [page, setPage] = useState(0);
    if (!warnings.length) return null;
    const current = Math.min(page, Math.floor((warnings.length - 1) / 25));
    return <details><summary>{t('Review warnings ({0})', warnings.length)}</summary>
        <ul className="curated-candidates">{warnings.slice(current * 25, (current + 1) * 25).map((warning, index) => <li key={index}>
            {warning.name || warning.storyUuid ? <strong>{warning.name || warning.storyUuid}: </strong> : null}<OutcomeText text={warning.message}/>
        </li>)}</ul>
        {warnings.length > 25 && <nav className="pagination" aria-label={t('Warning pages')}><button type="button" disabled={current === 0} onClick={() => setPage(current - 1)}>{t('← Previous')}</button>
            <span>{t('Page {0} of {1}', current + 1, Math.ceil(warnings.length / 25))}</span><button type="button" disabled={(current + 1) * 25 >= warnings.length} onClick={() => setPage(current + 1)}>{t('Next →')}</button></nav>}
    </details>;
}
