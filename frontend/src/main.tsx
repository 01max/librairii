import React from 'react'
import {flushSync} from 'react-dom'
import {createRoot} from 'react-dom/client'
import './style.css'
import App from './App'
import {installExternalNavigationGuard} from './navigation-policy'

const container = document.getElementById('root')

const root = createRoot(container!)
installExternalNavigationGuard()

async function renderApplication() {
    if (
        (import.meta.env.DEV ||
            import.meta.env.VITE_INCLUDE_FIXTURES === '1') &&
        new URLSearchParams(window.location.search).has('fixture')
    ) {
        const {installCollectionFixture} = await import('./dev-fixture');
        installCollectionFixture();
    }

    flushSync(() => {
        root.render(
            <React.StrictMode>
                <App/>
            </React.StrictMode>,
        );
    });
    const wailsWindow = window as typeof window & {
        go?: {
            main?: {
                App?: {
                    FrontendRendered?: () => Promise<void>;
                };
            };
        };
    };
    void wailsWindow.go?.main?.App?.FrontendRendered?.();
}

void renderApplication();
