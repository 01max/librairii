import {type ComponentProps, useState} from 'react';
import {normalizeSearchText} from './query-codec';

// Keep the editable text separate from the canonical query used by history and
// matching. Trimming a controlled input on every keystroke prevents word spaces.
export default function SearchInput({value, onChange, ...props}: Omit<ComponentProps<'input'>, 'value' | 'onChange'> & {
    value: string;
    onChange: (text: string) => void;
}) {
    const [draft, setDraft] = useState({text: value, canonical: value});
    if (draft.canonical !== value) setDraft({text: value, canonical: value});
    return <input {...props} type="search" value={draft.canonical === value ? draft.text : value}
        onChange={(event) => {
            const text = event.currentTarget.value;
            setDraft({text, canonical: normalizeSearchText(text)});
            onChange(text);
        }}/>;
}
