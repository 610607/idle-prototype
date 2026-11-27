// Mirrors engine/src/client/view.rs, api.rs and combat.rs. The client renders these; it never decides rules.

export type Tag = 'shock' | 'mark' | 'overload' | 'erosion' | 'shield' | 'fortify';
export const TAGS: Tag[] = ['shock', 'mark', 'overload', 'erosion', 'shield', 'fortify'];

export interface Resources {
  scrap: number;
  parts: number;
  intel: number;
  crystal: number;
}

export type Outcome = 'victory' | 'planned' | 'emergency' | 'wipe';
export type Dial = 'mine' | 'balance' | 'seal';
export type Phase = 'choosing_map' | 'active';

export interface TraitView {
  id: string;
  name: string;
  icon: string;
  desc: string;
  reads: Tag[];
  writes: Tag[];
  exp: boolean;
}

export interface SkillView {
  id: string;
  name: string;
  icon: string;
  passive: boolean;
  desc: string;
  reads: Tag[];
  writes: Tag[];
  trigger?: string;
}

export interface GrowingView {
  id: string;
  name: string;
  hint: string;
  have: number;
  need: number;
}

export interface ItemView {
  id: string;
  name: string;
  icon: string;
  rarity: number;
  desc: string;
  reads: Tag[];
  writes: Tag[];
}

export interface Derived {
  hp: number;
  atk: number;
  def: number;
  spd: number;
  ap: number;
  pp: number;
  tactic_slots: number;
}

export type Cond =
  | { t: 'always' }
  | { t: 'self_hp_below'; pct: number }
  | { t: 'self_hp_above'; pct: number }
  | { t: 'ally_hp_below'; pct: number }
  | { t: 'enemy_has_tag'; tag: Tag }
  | { t: 'self_has_tag'; tag: Tag }
  | { t: 'enemy_tag_at_least'; tag: Tag; n: number }
  | { t: 'any_tag_at_least'; n: number }
  | { t: 'enemy_count_at_least'; n: number }
  | { t: 'wave_at_least'; n: number }
  | { t: 'round_at_least'; n: number };

export type Act = { t: 'skill'; skill: string } | { t: 'retreat_planned' } | { t: 'retreat_emergency' };

export type Target =
  | 'default'
  | 'front_lowest'
  | 'back_first'
  | 'highest_atk'
  | 'with_tag'
  | 'lowest_ally'
  | 'self'
  | 'most_tags';

export interface Gambit {
  if: Cond;
  and?: Cond | null;
  do: Act;
  target: Target;
  on: boolean;
}

export type Assignment = { t: 'idle' } | { t: 'squad'; squad: number } | { t: 'garrison'; tile: number };

export interface OpView {
  id: number;
  name: string;
  class: string;
  class_name: string;
  icon: string;
  side: string;
  front: boolean;
  rarity: number;
  rarity_name: string;
  origin: string;
  stats: number[];
  derived: Derived;
  traits: TraitView[];
  growing: GrowingView[];
  skills: SkillView[];
  gambits: Gambit[];
  assignment: Assignment;
  status: string;
  equipment?: ItemView;
  special?: string;
  locked: boolean;
  reads: Tag[];
  writes: Tag[];
}

export interface CandidateView {
  id: number;
  source: string;
  arrived: number;
  op: OpView;
  cost: number;
}

export interface RiftView {
  kind: string;
  name: string;
  resident: string;
  dial: Dial;
  residents: number;
  talent_pct: number;
}

export interface EnemyBrief {
  id: string;
  name: string;
  icon: string;
  writes: Tag[];
}

export type SpecialKind = 'landmark' | 'cache' | 'clue' | 'core';

export interface TileView {
  i: number;
  label: string;
  terrain: 'unknown' | 'hq' | 'city' | 'rift' | 'special' | 'barrier';
  ring: number;
  revealed: boolean;
  owned: boolean;
  warden: number;
  alert: boolean;
  claimable: boolean;
  border: boolean;
  district?: number;
  usage: string;
  frozen?: string;
  garrison?: number;
  rift?: RiftView;
  special?: SpecialKind;
  special_done: boolean;
  natives: number;
  next_surge?: number;
  surge_source?: number;
  enemies: EnemyBrief[][];
  squads: number[];
  workers?: [number, number];
}

export interface TalentView {
  prof: string;
  name: string;
  icon: string;
  workers: number;
  jobs: number;
  pct: number;
  rate: number;
  eta_hours: number | null;
  instructors: number;
}

export interface PolicyOption {
  id: string;
  name: string;
  desc: string;
  available: boolean;
  reason?: string;
}

export interface DistrictView {
  id: number;
  name: string;
  tiles: number[];
  owned: number;
  total: number;
  contiguous: boolean;
  trust: number;
  trust_target: number;
  trust_factors: [string, number][];
  population: number;
  housing: number;
  unhoused: number;
  employed: number;
  militia: number;
  volunteers: number;
  talent: TalentView[];
  training_bp: number;
  policy: string | null;
  policy_unlocked: boolean;
  policy_options: PolicyOption[];
  priority: string[];
  flavor: TraitView;
  /** Residents gained per hour x100 (0 when housing is full). */
  growth_x100: number;
  customize: CustomizeView;
}

/** 定制招募 for one district: what can be ordered here and what it costs (in residents). */
export interface CustomizeView {
  unlocked: boolean;
  hint: string;
  base: number;
  class_cost: number;
  lock_cost: number;
  rarity_cost: [number, number, number, number];
  options: CustomizeOption[];
}

export interface CustomizeOption {
  prof: string;
  prof_name: string;
  prof_icon: string;
  workers: number;
  ok: boolean;
  reason: string | null;
  classes: CustomizeClass[];
}

export interface CustomizeClass {
  id: string;
  name: string;
  icon: string;
  mixed: boolean;
  ok: boolean;
  reason: string | null;
  traits: TraitView[];
}

export interface TileResultView {
  tile: number;
  label: string;
  outcome: Outcome;
  report: number | null;
}

export interface TripView {
  start: number;
  end: number;
  outcome: Outcome;
  walked: number[];
  results: TileResultView[];
  loot: Resources;
  drops: string[];
}

export interface SquadView {
  id: number;
  name: string;
  members: number[];
  route: number[];
  running: TripView | null;
  last: TripView | null;
  rest_until: number;
  trips: number;
  deepest: number;
}

export interface FacilityOption {
  id: string;
  name: string;
  icon: string;
  kind: 'production' | 'defense' | 'service' | 'training';
  profession: string | null;
  jobs: number;
  power: number;
  cost: Resources;
  desc: string;
  visible: boolean;
  affordable: boolean;
}

export interface TideView {
  t: number;
  tile: number;
  label: string;
  held: boolean;
  lost_wave: number | null;
  total_waves: number;
  warden_before: number;
  warden_after: number;
  source: number;
  skipped: boolean;
}

export interface PendingView {
  kind: 'map' | 'alert' | 'alert_more' | 'claim' | 'candidate' | 'policy' | 'route' | 'defend' | 'defend_more' | 'research';
  text: string;
  tile?: number;
  op?: number;
  candidate?: number;
  district?: number;
}

export interface GameEvent {
  id: number;
  t: number;
  kind: string;
  text: string;
  tile?: number;
  op?: number;
  report?: number;
}

export interface SpecialView {
  id: string;
  name: string;
  side: string;
  rule: string;
  hint: string;
  have: number;
  need: number;
  met: boolean;
  recruited: boolean;
  icon: string;
}

export interface OrgView {
  level: number;
  exp: number;
  next: number | null;
  cycles: number;
  rules: { id: string; name: string; desc: string; on: boolean }[];
  specials: SpecialView[];
  perks: { level: number; text: string; unlocked: boolean }[];
  history: {
    number: number;
    modifier: string;
    exp: number;
    management: number;
    defense: number;
    exploration: number;
    ended_at: number;
  }[];
  sealed_slots: number;
}

export interface PreviewView {
  index: number;
  modifier: string;
  modifier_name: string;
  modifier_desc: string;
  barriers: number[];
  rifts: number;
  specials: number;
  districts: number;
}

export interface CycleResults {
  management: number;
  defense: number;
  exploration: number;
  total: number;
  lines: [string, number][];
}

export interface View {
  time: number;
  revision: number;
  phase: Phase;
  cycle: number;
  started_at: number;
  modifier: [string, string, string] | null;
  previews: PreviewView[];
  resources: Resources;
  income: [number, number, number, number];
  power: [number, number, number];
  attention: [number, number, number];
  self_sufficient: boolean;
  border_count: number;
  tiles: TileView[];
  districts: DistrictView[];
  squads: SquadView[];
  operatives: OpView[];
  candidates: CandidateView[];
  queue: number;
  roster_cap: number;
  inventory: ItemView[];
  sealed: ItemView[];
  facilities: FacilityOption[];
  tide: TideView[];
  pending: PendingView[];
  events: GameEvent[];
  org: OrgView;
  results: CycleResults;
  hire_cost: number;
  research: ResearchView;
  tasks: TasksView;
}

export interface TaskView {
  id: string;
  group: string;
  name: string;
  desc: string;
  reward: Resources;
  done: boolean;
}

export interface TasksView {
  open: TaskView[];
  claimed: number;
  total: number;
}

export interface DoctrineView {
  id: string;
  name: string;
  icon: string;
  rarity: number;
  desc: string;
  held: boolean;
  archived: boolean;
  new: boolean;
  links_held: string[];
}

export interface TagChip {
  id: string;
  name: string;
  icon: string;
}

export interface CodexEntry {
  id: string;
  name: string;
  a: TagChip;
  b: TagChip;
  seen: boolean;
  text: string;
  total: number;
  this_cycle: number;
}

export interface ResearchView {
  cost: number;
  step: number;
  reroll_cost: number;
  reroll_ok: boolean;
  affordable: boolean;
  left: number;
  offers: DoctrineView[];
  held: DoctrineView[];
  archive: DoctrineView[];
  archive_slots: number;
  library: DoctrineView[];
  reactions_on: boolean;
  codex: CodexEntry[];
  known: number;
  total: number;
}

// ---- api.rs ----

export interface Delta {
  label: string;
  before: string;
  after: string;
  better: boolean;
}

export interface DryRun {
  ok: boolean;
  error?: string;
  irreversible: boolean;
  deltas: Delta[];
  tide_before: TideView[];
  tide_after: TideView[];
}

export interface GarrisonOption {
  op: number | null;
  name: string;
  icon: string;
  current: boolean;
  busy: string | null;
  held: boolean;
  lost_wave: number | null;
  total_waves: number;
  rounds: number;
  hp_left_pct: number;
  warden_gain: number;
}

export interface RoutePreview {
  ok: boolean;
  error?: string;
  steps: { tile: number; label: string; fought: boolean; outcome: Outcome | null; rounds: number }[];
  outcome: Outcome | null;
  minutes: number;
  loot: Resources;
  drops: string[];
  attention_before: number;
  attention_after: number;
}

// ---- combat.rs ----

export interface SnapUnit {
  name: string;
  icon: string;
  side: 'ally' | 'enemy';
  front: boolean;
  hp: number;
  max_hp: number;
  shield: number;
  pp: number;
  tags: number[];
  alive: boolean;
  fixture?: boolean;
}

export interface LogEntry {
  id: number;
  cause?: number;
  depth: number;
  wave: number;
  round: number;
  kind: 'wave' | 'round' | 'action' | 'reaction' | 'dot' | 'tag' | 'extra' | 'facility' | 'death' | 'retreat' | 'end';
  actor?: string;
  target?: string;
  source?: string;
  amount: number;
  text: string;
  formula?: string;
  tags?: Tag[];
  snap?: SnapUnit[];
}

export interface UnitReport {
  key: string;
  name: string;
  icon: string;
  side: 'ally' | 'enemy';
  alive: boolean;
  hp: number;
  max_hp: number;
  dealt: number;
  taken: number;
  healed: number;
  reactions: number;
  tags_received: number[];
  tags_consumed: number;
}

export interface BattleReport {
  kind: 'route' | 'surge' | 'core' | 'counter';
  outcome: Outcome;
  waves_cleared: number;
  total_waves: number;
  rounds: number;
  lost_wave?: number;
  yield_bp: number;
  applied: number[];
  consumed: number[];
  warden_gain: number;
  /** Tag reactions set off, by id (only when the reaction doctrine is held). */
  reactions?: [string, number][];
  units: UnitReport[];
  log: LogEntry[];
}

export interface TagDef {
  id: Tag;
  name: string;
  icon: string;
  max: number;
  desc: string;
}

/** The numbers from engine/content/balance.json that the web shows in its own text. */
export interface Balance {
  surge_interval: number;
  tide_horizon: number;
  trust_policy: number;
  contiguity_trust: number;
  contiguity_output_bp: number;
  housing_capacity: number;
  warden_max: number;
  warden_route_win: number;
  warden_counter_restore: number;
  warden_counter_restore_clinic: number;
  org_perks: { archive: number[] };
}

export interface ContentView {
  tags: TagDef[];
  professions: { id: string; name: string; icon: string }[];
  facilities: unknown[];
  stats: [string, string][];
  skills: SkillView[];
  classes: [string, string, string][];
  policies: { id: string; name: string; desc: string }[];
  rift_kinds: { id: string; name: string; icon: string; resident: string }[];
}

export type Command =
  | { cmd: 'choose_map'; index: number }
  | { cmd: 'set_route'; squad: number; route: number[] }
  | { cmd: 'set_squad'; squad: number; members: number[] }
  | { cmd: 'garrison'; op: number; tile: number | null }
  | { cmd: 'claim'; tile: number }
  | { cmd: 'build'; tile: number; facility: string | null }
  | { cmd: 'counter'; tile: number }
  | { cmd: 'abandon'; tile: number }
  | { cmd: 'set_dial'; tile: number; dial: Dial }
  | { cmd: 'hire'; candidate: number }
  | { cmd: 'dismiss'; candidate: number }
  | { cmd: 'customize'; district: number; profession: string; class: string | null; lock: string | null; rarity: number | null }
  | { cmd: 'retire'; op: number }
  | { cmd: 'set_policy'; district: number; policy: string | null }
  | { cmd: 'set_priority'; district: number; priority: string[] }
  | { cmd: 'set_rule'; rule: string; on: boolean }
  | { cmd: 'set_gambits'; op: number; gambits: Gambit[] }
  | { cmd: 'equip'; op: number; item: string | null }
  | { cmd: 'seal'; item: string; on: boolean }
  | { cmd: 'lock'; op: number; locked: boolean }
  | { cmd: 'research' }
  | { cmd: 'reroll' }
  | { cmd: 'claim_task'; task: string }
  | { cmd: 'pick_doctrine'; doctrine: string }
  | { cmd: 'archive'; doctrine: string; on: boolean }
  | { cmd: 'collapse' };
