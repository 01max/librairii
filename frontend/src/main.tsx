import React from 'react'
import {createRoot} from 'react-dom/client'
import './style.css'
import App from './App'

const container = document.getElementById('root')

const root = createRoot(container!)

async function renderApplication() {
    if (
        import.meta.env.DEV &&
        new URLSearchParams(window.location.search).has('fixture')
    ) {
        const {installCollectionFixture} = await import('./dev-fixture');
        installCollectionFixture();
    }

    root.render(
        <React.StrictMode>
            <App/>
        </React.StrictMode>,
    );
}

void renderApplication();
