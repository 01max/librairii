import {useEffect, useState} from 'react';
import './App.css';
import {ApplicationStatus} from '../wailsjs/go/main/App';

function App() {
    const [state, setState] = useState('initializing');

    useEffect(() => {
        void ApplicationStatus().then((response) => {
            setState(response.error ? response.error.code : response.status.state);
        });
    }, []);

    return (
        <main id="App">
            <h1>Librairii</h1>
            <p data-testid="application-state">Application state: {state}</p>
        </main>
    );
}

export default App;
