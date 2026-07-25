import {render, screen} from '@testing-library/react';
import {beforeEach, expect, test, vi} from 'vitest';
import {ApplicationStatus} from '../wailsjs/go/main/App';
import {app} from '../wailsjs/go/models';
import App from './App';

vi.mock('../wailsjs/go/main/App', () => ({
    ApplicationStatus: vi.fn(),
}));

const applicationStatus = vi.mocked(ApplicationStatus);

beforeEach(() => {
    applicationStatus.mockReset();
});

test('renders the ready lifecycle state from the typed facade', async () => {
    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'ready',
        },
    }));

    render(<App/>);

    expect(await screen.findByText('Application state: ready')).toBeInTheDocument();
});

test('renders a stable error code without exposing backend details', async () => {
    applicationStatus.mockResolvedValue(new app.StatusResponse({
        status: {
            state: 'recovery',
        },
        error: {
            code: 'storage_unavailable',
            message: 'Storage is unavailable.',
        },
    }));

    render(<App/>);

    expect(await screen.findByText('Application state: storage_unavailable')).toBeInTheDocument();
});
