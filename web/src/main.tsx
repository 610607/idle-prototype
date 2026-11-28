import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Game } from './engine/client';
import { t } from './i18n';
import { App } from './ui/App';
import { StartScreen } from './ui/Start';
import './style.css';

// index.html carries the same title for the moment before this script runs.
document.title = t('app.title');
const root = createRoot(document.getElementById('root')!);

/** The game, or the start screen while there is none (first visit, or after 重新开始). */
function Root({ first }: { first: Game | null }) {
  const [game, setGame] = useState<Game | null>(first);
  // A new game is a new App, so nothing from the old one (open pages, timers) carries over.
  const [n, setN] = useState(0);
  if (!game)
    return (
      <StartScreen
        error={Game.bootError}
        onStart={(map) => {
          setGame(Game.start(map));
          setN((k) => k + 1);
        }}
      />
    );
  return (
    <App
      key={n}
      game={game}
      onRestart={() => {
        game.discard();
        setGame(null);
      }}
    />
  );
}

Game.boot()
  .then((game) => {
    root.render(
      <StrictMode>
        <Root first={game} />
      </StrictMode>,
    );
  })
  .catch((e) => {
    root.render(<pre style={{ padding: 16, color: '#b4372a' }}>{t('app.boot_failed', { err: String(e) })}</pre>);
  });
