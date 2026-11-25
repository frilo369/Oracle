/** Tencent's Mayhem wire format and English game metadata. No translated names. */
const ASSETS =
  'https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/';
const HAN = /[\u3400-\u9fff]/u;

export function plainText(value) {
  return String(value ?? '')
    .replace(/<br\s*\/?\s*>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .trim();
}

// The English metadata can contain unexpanded game templates (including a
// zero cooldown placeholder). Do not guess their live, mode-specific values.
export function itemDescription(value) {
  return plainText(value)
    .replace(/\(0(?:\.0+)?s\)/g, '(cooldown unavailable)')
    .replace(/\{\{[^{}]+\}\}|@[a-zA-Z][a-zA-Z0-9_.]*@/g, 'unavailable');
}

function english(value) {
  const text = plainText(value);
  return text && !HAN.test(text) ? text : '';
}

function asset(path) {
  const relative = String(path ?? '').replace(/^\/lol-game-data\/assets\//i, '');
  return relative && !relative.includes('..') && /^[a-z0-9_./-]+$/i.test(relative)
    ? ASSETS + relative.toLowerCase()
    : '';
}

function positiveId(value) {
  const n = Number(value);
  return Number.isSafeInteger(n) && n > 0 ? n : null;
}

// Tencent's own copy of an augment picture, as its Mayhem list names it.
const TENCENT_ART = /^https:\/\/game\.gtimg\.cn\/images\/lol\/act\/img\/rune\/[a-z0-9_]+\.png$/;
// The guide window serves League's own copy of a game picture here (src/guide.cjs).
const LOCAL_ART = './art/';
const RARITY = { kSilver: 'Silver', kGold: 'Gold', kPrismatic: 'Prismatic' };
const compact = (value) => String(value ?? '').toLowerCase().replace(/[^a-z0-9]/g, '');
/** The name a picture file gives, without its size, prefixes or punctuation: "ARAM_Eureka_small.png" is "eureka". */
function pictureName(url) {
  const file = String(url ?? '').split('/').pop().replace(/\.(?:png|jpg)$/i, '').replace(/_(?:small|large)$/i, '');
  return compact(file).replace(/^(?:aram|quest)/, '');
}
/** Every name an augment goes by: "Archmage" and "ARAM_Archmage" give archmage and aramarchmage. */
function augmentNames(augment) {
  return [...new Set([compact(augment.name), compact(augment.apiName), compact(augment.apiName).replace(/^aram/, '')])].filter((name) => name.length >= 3);
}
const generic = (url) => /genericabilityaugmenticon/i.test(String(url ?? ''));
const ownNames = new WeakMap();
function allAugmentNames(catalogue) {
  let names = ownNames.get(catalogue.augments);
  if (!names) {
    names = new Set(Object.values(catalogue.augments ?? {}).flatMap(augmentNames));
    ownNames.set(catalogue.augments, names);
  }
  return names;
}
/** The installed League's copy first, then CommunityDragon's: one picture's addresses in order. */
function sources(url) {
  return url.startsWith(ASSETS) ? [LOCAL_ART + url.slice(ASSETS.length), url] : url ? [url] : [];
}

const large = (url) => url.replace(/_small\.png$/i, '_large.png');

/**
 * Every address to try for an entry's picture, best first (9.5, owner: "augments show wrong icons").
 * Riot's data names each augment's 64 px "small" picture, which for most is only its category's glyph,
 * so one wand stood for every magic augment. The augment's own art is its "large" picture: League's
 * copy, else Tencent's (League's client ships few). Riot's data also gives some augments a placeholder
 * or another augment's picture (Archmage has Eureka's): for those, only art named after the augment
 * itself is tried before Riot's choice.
 */
export function artSources(catalogue, kind, entry) {
  const riot = sources(entry?.icon ?? '');
  if (kind !== 'augments' || !entry?.icon) return riot;
  const names = augmentNames(entry), shown = pictureName(entry.icon);
  const borrowed = generic(entry.icon) || (!names.includes(shown) && allAugmentNames(catalogue).has(shown));
  const fits = (url) => names.includes(pictureName(url)) || (!borrowed && pictureName(url) === shown);
  const tencent = (entry.tencentArt ?? []).filter(fits);
  if (!borrowed) return [...new Set([...sources(large(entry.icon)).slice(0, 1), ...tencent, ...riot])];
  const named = (size) => names.flatMap((name) => ['kiwi', 'cherry'].map((folder) => `${LOCAL_ART}assets/ux/${folder}/augments/icons/${name}_${size}.png`));
  return [...new Set([...named('large'), ...tencent, ...named('small'), ...riot])];
}

export function normaliseCatalogue({ champions = [], items = [], augments = [], tencentAugments = [] } = {}) {
  // Tencent's art for each augment, large first.
  const tencentArt = new Map();
  for (const row of Array.isArray(tencentAugments) ? tencentAugments : []) {
    const id = positiveId(row?.augmentID);
    const art = [...new Set([row?.large_Icon, row?.small_Icon].map((url) => String(url ?? '')).filter((url) => TENCENT_ART.test(url)))];
    if (id && art.length) tencentArt.set(id, art);
  }
  const result = { champions: {}, items: {}, augments: {} };
  for (const row of champions) {
    const id = positiveId(row.id),
      name = english(row.name);
    // Jade has separate 60000+ character IDs; those are not Mayhem champions.
    if (id && id <= 10000 && name)
      result.champions[id] = {
        id,
        name,
        alias: english(row.alias),
        icon: asset(row.squarePortraitPath),
      };
  }
  for (const row of items) {
    const id = positiveId(row.id),
      name = english(row.name);
    if (id && name)
      result.items[id] = {
        id,
        name,
        icon: asset(row.iconPath),
        description: english(row.description),
      };
  }
  for (const row of augments) {
    const id = positiveId(row.id),
      name = english(row.nameTRA);
    if (id && name)
      result.augments[id] = {
        id,
        name,
        apiName: String(row.augmentNameId ?? ''),
        icon: asset(row.augmentSmallIconPath),
        ...(tencentArt.has(id) ? { tencentArt: tencentArt.get(id) } : {}),
        rarity: RARITY[row.rarity] ?? '',
      };
  }
  if (
    Object.keys(result.champions).length < 100 ||
    Object.keys(result.items).length < 100 ||
    Object.keys(result.augments).length < 100
  )
    throw new Error('The English game catalogue is incomplete.');
  return result;
}

/** Nested JSON inside _fieldValues is sometimes escaped a second time. */
export function decodeJson(value) {
  for (let depth = 0; depth < 5 && typeof value === 'string'; depth++) {
    try {
      value = JSON.parse(value);
    } catch {
      const unescaped = value.replace(/\\"/g, '"').replace(/\\\\/g, '\\');
      if (unescaped === value) throw new Error('Tencent returned unreadable Mayhem data.');
      value = unescaped;
    }
  }
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Tencent returned an empty Mayhem response.');
  return value;
}

export function tencentPayload(response) {
  let value = decodeJson(response);
  for (let depth = 0; depth < 6; depth++) {
    if (
      (value.code != null && Number(value.code) !== 0) ||
      (typeof value.result === 'number' && value.result !== 0)
    )
      throw new Error('Tencent could not provide Mayhem data.');
    if (value.data?._fieldValues) {
      const fields = Object.values(value.data._fieldValues);
      if (fields.length !== 1) throw new Error('Tencent returned an unexpected Mayhem response.');
      value = decodeJson(fields[0]);
    } else if ('data' in value) value = decodeJson(value.data);
    else if ('result' in value && typeof value.result !== 'number')
      value = decodeJson(value.result);
    else return value;
  }
  throw new Error('Tencent returned an unexpected Mayhem response.');
}

export function percent(value, scale = 'ratio') {
  if (
    value == null ||
    String(value).trim() === '' ||
    !/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(String(value))
  )
    return null;
  const n = Number(value) * (scale === 'basisPoints' ? 0.01 : 100);
  return Number.isFinite(n) && n >= 0 && n <= 100 ? Math.round(n * 1e6) / 1e6 : null;
}

export function formatPercent(value) {
  return value == null || !Number.isFinite(value) ? '—' : `${value.toFixed(2)}%`;
}

function date(value) {
  const text = String(value ?? '');
  if (!/^\d{8}$/.test(text)) throw new Error('Tencent did not provide the Mayhem data date.');
  const formatted = `${text.slice(0, 4)}-${text.slice(4, 6)}-${text.slice(6, 8)}`;
  const parsed = new Date(`${formatted}T00:00:00Z`);
  if (!Number.isFinite(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== formatted)
    throw new Error('Tencent returned an invalid data date.');
  return formatted;
}

function rows(value) {
  if (!value) return [];
  try {
    return Object.values(decodeJson(value));
  } catch {
    return [];
  }
}

function ids(value, separator = /[,&]/) {
  return String(value ?? '')
    .split(separator)
    .map(positiveId)
    .filter((id) => id !== null);
}

function equipment(value, kind) {
  if (kind === 'starting' || kind === 'boots')
    return String(value ?? '')
      .split('#')
      .filter(Boolean)
      .flatMap((row, index) => {
        const [items, pick, win] = row.split('$');
        const itemIds = ids(items);
        return itemIds.length
          ? [{ id: index + 1, itemIds, pickRate: percent(pick), winRate: percent(win) }]
          : [];
      });
  return rows(value).flatMap((row, index) => {
    const itemIds = ids(kind === 'core' ? row.itemcore : row.itemone);
    return itemIds.length
      ? [
          {
            id: index + 1,
            itemIds,
            pickRate: percent(row.showrate, 'basisPoints'),
            winRate: percent(row.winrate, 'basisPoints'),
          },
        ]
      : [];
  });
}

const SKILLS = { 1: 'Q', 2: 'W', 3: 'E', 4: 'R' };
// Highest reported win rate first; pick rate breaks ties. Missing rates sort last.
export function sortSkillOrders(orders) {
  const rate = value => Number.isFinite(value) ? value : -1;
  return [...orders].sort((a,b) => rate(b.winRate)-rate(a.winRate) || rate(b.pickRate)-rate(a.pickRate));
}
function skills(value) {
  return String(value ?? '')
    .split('&')
    .map((key) => SKILLS[key])
    .filter(Boolean);
}

export function parseMayhemBuild(response, championId) {
  const data = tencentPayload(response);
  const dataDate = date(data.dtstatdate);
  if (!positiveId(championId)) throw new Error('Choose a valid champion.');
  const rarities = new Map();
  for (const group of String(data.augment_json ?? '').split('&')) {
    const content = group.includes(':') ? group.slice(group.indexOf(':') + 1) : group;
    for (const line of content.split('#')) {
      const [, , id, level] = line.split('|');
      const rarity = { kSilver: 'Silver', kGold: 'Gold', kPrismatic: 'Prismatic' }[level];
      if (positiveId(id) && rarity) rarities.set(Number(id), rarity);
    }
  }
  const augments = new Map();
  for (const group of String(data.augment_json_irank || data.augment_json || '').split('&')) {
    const content = group.includes(':') ? group.slice(group.indexOf(':') + 1) : group;
    for (const line of content.split('#').filter(Boolean)) {
      const [rank, owner, augment, rarity, pick, grade] = line.split('|');
      const id = positiveId(augment);
      if (!id || !positiveId(rank)) continue;
      if (Number(owner) !== Number(championId))
        throw new Error('Tencent returned a build for a different champion.');
      if (!augments.has(id))
        augments.set(id, {
          id,
          rank: Number(rank),
          rarity:
            { kSilver: 'Silver', kGold: 'Gold', kPrismatic: 'Prismatic' }[rarity] ??
            rarities.get(id) ??
            '',
          pickRate: percent(pick),
          grade: /^[SABCD]$/.test(grade ?? '') ? grade : '',
        });
    }
  }
  const sections = [
    { id: 'core', title: 'Core Builds', rows: equipment(data.itemcore_json, 'core') },
    { id: 'situational', title: 'Other Items', rows: equipment(data.itemone_json, 'situational') },
    { id: 'boots', title: 'Boots', rows: equipment(data.itemshoes, 'boots') },
  ];
  const skillOrders = sortSkillOrders(rows(data.skill_json).flatMap((row, index) => {
    const priority = skills(row.qwe);
    return priority.length
      ? [
          {
            id: index + 1,
            priority,
            pickRate: percent(row.qwe_s, 'basisPoints'),
            winRate: percent(row.qwe_w, 'basisPoints'),
            plans: sortSkillOrders(rows(row.sks)
              .map((plan) => ({
                order: skills(plan.sk),
                pickRate: percent(plan.sk_s, 'basisPoints'),
                winRate: percent(plan.sk_w, 'basisPoints'),
              }))
              .filter((plan) => plan.order.length)),
          },
        ]
      : [];
  }));
  if (!augments.size && !sections.some((section) => section.rows.length))
    throw new Error('Tencent has no Mayhem build for this champion yet.');
  return {
    championId: Number(championId),
    dataDate,
    augments: [...augments.values()],
    sections,
    skillOrders,
  };
}

/** Every rarity remains visible; search applies to all three sections together. */
export function groupMayhemAugments(augments, catalogue, query = '') {
  const text = query.trim().toLocaleLowerCase('en');
  return ['Silver', 'Gold', 'Prismatic'].map((rarity) => ({
    rarity,
    rows: augments
      .filter((row) => (row.rarity || catalogue.augments[row.id]?.rarity) === rarity)
      .filter((row) =>
        resolveGameEntry(catalogue, 'augments', row.id).name.toLocaleLowerCase('en').includes(text),
      ),
  }));
}

export function parseMayhemRankings(response, requestedDate) {
  const data = tencentPayload(response);
  const dataDate = date(data.dtstatdate || requestedDate);
  if (requestedDate && dataDate !== date(requestedDate))
    throw new Error('The Mayhem ranking and build dates do not match.');
  const champions = String(data.listcollect ?? '')
    .split('#')
    .filter(Boolean)
    .flatMap((line) => {
      const fields = line.split('_');
      const id = positiveId(fields[0]),
        rank = positiveId(fields[1]);
      return id && rank
        ? [{ id, rank, winRate: percent(fields[3]), pickRate: percent(fields[4]) }]
        : [];
    });
  if (!champions.length) throw new Error('Tencent has no Mayhem rankings for this date.');
  return { dataDate, champions };
}

export function resolveGameEntry(catalogue, kind, id) {
  return (
    catalogue[kind]?.[id] ?? {
      id,
      name: `${{ items: 'Item', augments: 'Augment', champions: 'Champion' }[kind]} ${id} · English name unavailable`,
      icon: '',
      description: '',
      rarity: '',
    }
  );
}

export function sortEquipment(rows, sort = 'winRate') {
  return [...rows].sort((a, b) => (b[sort] ?? -1) - (a[sort] ?? -1) || a.id - b.id);
}
