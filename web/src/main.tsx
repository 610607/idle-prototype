import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Game } from './engine/client';
import { t } from './i18n';
import { App } from './ui/App';
import './style.css';

// index.html carries the same title for the moment before this script runs.
document.title = t('app.title');
const root = createRoot(document.getElementById('root')!);

Game.boot()
  .then((game) => {
    root.render(
      <StrictMode>
        <App game={game} />
      </StrictMode>,
    );
  })
  .catch((e) => {
    root.render(<pre style={{ padding: 16, color: '#b4372a' }}>{t('app.boot_failed', { err: String(e) })}</pre>);
  });
