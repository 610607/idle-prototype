// Player-facing text lives in locales/<lang>/ (docs/i18n.md), never in components. The web reads
// its own web.json plus the words it shares with the engine (engine.json). `{name}` in a string
// is replaced by the argument of that name; a missing key shows up as itself.
//
//   t('tile.claim')                       → "认领"
//   t('tile.cost', { n: 3 })              → "花费 3"
//   tn('tile.held_by', { name: <b>…</b> }) → React nodes, for text with elements inside
import { createElement, Fragment, type ReactNode } from 'react';
import engine from '../../locales/zh-CN/engine.json';
import web from '../../locales/zh-CN/web.json';

export type Key = keyof typeof web | keyof typeof engine;
type Entry = string | string[];

const catalog: Record<string, Entry> = { ...engine, ...web };

/** Only `{identifier}` is a placeholder, so text like `{"cmd":"claim"}` passes through. */
const PLACEHOLDER = /\{([A-Za-z0-9_]+)\}/g;

function template(key: string): string {
  const e = catalog[key];
  if (e === undefined) return key;
  return Array.isArray(e) ? e.join(', ') : e;
}

export function has(key: string): key is Key {
  return key in catalog;
}

export function t(key: Key, args?: Record<string, string | number>): string {
  const s = template(key);
  if (!args) return s;
  return s.replace(PLACEHOLDER, (m, name: string) => (name in args ? String(args[name]) : m));
}

/** `t` whose arguments may be elements; returns one fragment, so no keys are needed. */
export function tn(key: Key, args: Record<string, ReactNode>): ReactNode {
  const s = template(key);
  const parts: ReactNode[] = [];
  let last = 0;
  for (const m of s.matchAll(PLACEHOLDER)) {
    parts.push(s.slice(last, m.index));
    parts.push(m[1] in args ? args[m[1]] : m[0]);
    last = m.index + m[0].length;
  }
  parts.push(s.slice(last));
  return createElement(Fragment, null, ...parts);
}

/** A list entry (name pools, short word lists). */
export function list(key: Key): string[] {
  const e = catalog[key];
  return Array.isArray(e) ? e : e === undefined ? [] : [e];
}
