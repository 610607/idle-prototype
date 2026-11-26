import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Game } from './engine/client';
import { App } from './ui/App';
import './style.css';

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
    root.render(<pre style={{ padding: 16, color: '#b4372a' }}>引擎加载失败：{String(e)}</pre>);
  });
