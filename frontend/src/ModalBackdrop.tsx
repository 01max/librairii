import {type PropsWithChildren, useRef} from 'react';

export default function ModalBackdrop({
    children, onDismiss, disabled = false, className = '',
}: PropsWithChildren<{onDismiss: () => void; disabled?: boolean; className?: string}>) {
    const startedOutside = useRef(false);
    return (
        <div
            className={`dialog-backdrop ${className}`.trim()}
            onPointerDown={(event) => {
                startedOutside.current = event.target === event.currentTarget;
            }}
            onClick={(event) => {
                if (event.target === event.currentTarget && startedOutside.current && !disabled) {
                    event.stopPropagation();
                    onDismiss();
                }
            }}
        >
            {children}
        </div>
    );
}
