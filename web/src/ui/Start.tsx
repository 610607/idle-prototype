import { t } from '../i18n';
import { Icon } from '../icons';

/** No game yet (first visit, or after 重新开始): pick the tutorial (recommended) or a random map.
 *  Everything stays in the page; nothing depends on the address (static hosts drop query strings). */
export function StartScreen({ error, onStart }: { error: string | null; onStart: (map: string | null) => void }) {
  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">
          <span className="stamp">{t('app.brand.stamp')}</span>
          <h1>{t('app.brand.name')}</h1>
        </span>
      </header>
      <div className="work-body">
        <div className="pane start">
          <div>
            <h2 style={{ fontSize: 26 }}>{t('start.title')}</h2>
            <p className="muted">{t('start.intro')}</p>
          </div>
          {error ? <div className="card striped bad small">{t('start.load_error', { err: error })}</div> : null}
          <div className="start-choices">
            <div className="card rivet stack start-choice recommended">
              <div className="row" style={{ gap: 8 }}>
                <Icon id="tile-hq" size={32} />
                <h3 className="grow" style={{ fontSize: 20 }}>
                  {t('start.tutorial.name')}
                </h3>
                <span className="chip good">{t('start.recommended')}</span>
              </div>
              <p className="small">{t('start.tutorial.desc')}</p>
              <p className="small muted">{t('start.tutorial.then')}</p>
              <button className="btn primary" autoFocus onClick={() => onStart('tutorial')}>
                {t('start.tutorial.go')}
              </button>
            </div>
            <div className="card rivet stack start-choice">
              <div className="row" style={{ gap: 8 }}>
                <Icon id="map" size={32} />
                <h3 className="grow" style={{ fontSize: 20 }}>
                  {t('start.random.name')}
                </h3>
              </div>
              <p className="small">{t('start.random.desc')}</p>
              <p className="small muted">{t('start.random.then')}</p>
              <button className="btn" onClick={() => onStart(null)}>
                {t('start.random.go')}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
