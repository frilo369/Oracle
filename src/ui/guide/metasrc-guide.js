/* Trusted, self-contained WebView initialization code. No native IPC or remote
 * scripts are evaluated here. The factory is also used by the fixture tests. */
const metasrcGuide = function metasrcGuide() {
  const SCHEMA = 2;
  const HOST = 'www.metasrc.com';
  const MAX_AGE = 15 * 60000;
  const CACHE_KEY = 'fryiloco-metasrc-guides-v2';
  const text = (node) => (node?.textContent || '').replace(/\s+/g, ' ').trim();
  const escape = (value) =>
    String(value ?? '').replace(
      /[&<>"']/g,
      (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
    );
  const percent = (value) =>
    /^\d+(?:\.\d+)?%$/.test(String(value).trim()) && Number.parseFloat(value) <= 100
      ? Number.parseFloat(value)
      : null;
  const format = (value) => (Number.isFinite(value) ? `${Number(value.toFixed(2))}%` : '—');
  const tierScore = (value) =>
    ['S+', 'S', 'A+', 'A', 'B+', 'B', 'C+', 'C', 'D+', 'D', 'F'].indexOf(value);
  const slugFor = (name) =>
    String(name)
      .toLowerCase()
      .replace(/[^a-z0-9 -]/g, '')
      .trim()
      .replace(/[ -]+/g, '-');
  function sourceURL(value, base = `https://${HOST}`) {
    try {
      const url = new URL(value, base);
      if (
        url.protocol !== 'https:' ||
        url.hostname !== HOST ||
        url.port ||
        url.username ||
        url.password
      )
        return null;
      const match = url.pathname.match(
        /^\/lol\/(?:(arena|aram)\/)?champions\/([a-z0-9]+(?:-[a-z0-9]+)*)(?:\/(top|jungle|mid|adc|support))?\/build\/?$/,
      );
      if (!match || url.href.length > 2048) return null;
      url.hash = '';
      return { url: url.href, mode: match[1] || 'rift', slug: match[2], role: match[3] || '' };
    } catch {
      return null;
    }
  }
  function assetURL(value) {
    try {
      const url = new URL(value, `https://${HOST}`);
      return url.protocol === 'https:' &&
        url.hostname === 'cdn.metasrc.com' &&
        !url.username &&
        !url.password &&
        !url.port &&
        url.pathname.startsWith('/static/')
        ? url.href
        : '';
    } catch {
      return '';
    }
  }
  function apiURL(value, pageURL) {
    try {
      const url = new URL(value, pageURL);
      return url.origin === `https://${HOST}` &&
        !url.username &&
        !url.password &&
        !url.hash &&
        /^\/api\/lol\/(?:widgets\/|tooltip\/)/.test(url.pathname) &&
        url.href.length <= 2048
        ? url.href
        : '';
    } catch {
      return '';
    }
  }
  function augmentURL(value, pageURL) {
    const source = sourceURL(pageURL),
      target = apiURL(value, pageURL);
    if (!source || source.mode !== 'arena' || !target) return '';
    const url = new URL(target),
      page = new URL(source.url);
    if (url.pathname !== `/api/lol/widgets/arena/tools/${source.slug}/arena`) return '';
    return ['region', 'ranks', 'patch', 'build'].every(
      (key) => url.searchParams.get(key) === page.searchParams.get(key),
    )
      ? target
      : '';
  }
  const sorted = (rows, key) =>
    key === 'order'
      ? [...rows]
      : [...rows].sort((a, b) => {
          if (key === 'tier') {
            const aTier = tierScore(a.grade),
              bTier = tierScore(b.grade);
            const delta = (aTier < 0 ? 99 : aTier) - (bTier < 0 ? 99 : bTier);
            if (delta) return delta;
          } else {
            const av = a[key],
              bv = b[key];
            if (Number.isFinite(av) || Number.isFinite(bv)) {
              const delta = (Number.isFinite(bv) ? bv : -1) - (Number.isFinite(av) ? av : -1);
              if (delta) return delta;
            }
          }
          return (b.pickRate ?? -1) - (a.pickRate ?? -1);
        });
  function art(node, pageURL) {
    const icon = assetURL(
      node.getAttribute('src') ||
        node.getAttribute('data-xlink-href') ||
        node.getAttribute('href') ||
        node.getAttribute('xlink:href'),
    );
    if (!icon) return null;
    const trigger =
      node.closest('[data-tooltip-select]') ||
      node.parentElement?.querySelector('[data-tooltip-select]');
    const key =
      trigger?.getAttribute('data-tooltip-select')?.replace(/^#/, '') ||
      new URL(icon).pathname
        .split('/')
        .at(-1)
        .replace(/\.png$/, '');
    const fallback = new URL(icon).pathname
      .split('/')
      .at(-1)
      .replace(/-\d+\.png$/, '')
      .replace(/\.png$/, '')
      .replaceAll('-', ' ')
      .replace(/\b\w/g, (c) => c.toUpperCase());
    return {
      key,
      quantity:
        key.startsWith('item-') &&
        /^(?:[2-9]|[1-9]\d)$/.test(text(node.parentElement?.parentElement))
          ? Number(text(node.parentElement.parentElement))
          : 1,
      name: node.getAttribute('alt') || fallback,
      icon,
      tooltip: apiURL(trigger?.getAttribute('data-tooltip-url') || '', pageURL),
      target: key,
    };
  }
  function images(node, pageURL) {
    return [...node.querySelectorAll('img[src],svg image')]
      .map((n) => art(n, pageURL))
      .filter(Boolean);
  }
  function readStyles(header, href) {
    const source = sourceURL(href);
    return [...header.querySelectorAll('[data-tooltip-select^="#tab-"]')]
      .flatMap((node) => {
        const link = node.closest('[href]'),
          target = sourceURL(link?.getAttribute('href'), href);
        if (
          !target ||
          target.slug !== source.slug ||
          target.mode !== source.mode ||
          target.role !== source.role
        )
          return [];
        return [
          {
            id: link.id,
            url: target.url,
            key: new URL(target.url).searchParams.get('build') || 'all',
            label: text(node.querySelector('span')) || 'Recommended',
            items: images(node, href),
            selected: link.classList.contains('active'),
          },
        ];
      })
      .filter((row, index, rows) => rows.findIndex((other) => other.url === row.url) === index)
      .slice(0, 20);
  }
  function readPath(section, href) {
    if (!section) return [];
    return [...section.querySelectorAll('img[src]')].flatMap((node) => {
      const item = art(node, href);
      if (!item) return [];
      let parent = node.parentElement,
        cost = '';
      for (let depth = 0; parent && depth < 5; depth++, parent = parent.parentElement) {
        if (images(parent, href).length !== 1) break;
        const coin = parent.querySelector('img[alt="gold coin"]');
        if (coin) {
          cost = text(coin.parentElement).match(/^\d[\d,]*$/)?.[0] || '';
          break;
        }
      }
      return [{ ...item, cost }];
    });
  }
  function metricGroups(section, pageURL, arena) {
    if (!section) return [];
    return [...section.querySelectorAll('.group')].flatMap((group) => {
      if (group.closest('header')) return [];
      const values = [...group.querySelectorAll('span,strong')]
        .filter((n) => !n.children.length)
        .map(text);
      const rates = values.map(percent).filter((n) => n !== null);
      const items = images(group, pageURL);
      if (!rates.length || !items.length) return [];
      const grade = values.find((n) => /^(S\+?|[ABCDF]\+?)$/.test(n)) || '';
      return [
        {
          items,
          winRate: arena ? null : rates[0],
          pickRate: arena ? rates[0] : (rates[1] ?? null),
          grade,
          metric: /Pick\s*%/.test(text(section)) && !/Buy\s*%/.test(text(section)) ? 'pick' : 'buy',
          games: arena
            ? values[values.findIndex((n) => percent(n) !== null) + 1]?.match(
                /^\d[\d,.]*[KMB]?$/,
              )?.[0] || ''
            : '',
          label: /^\d+$/.test(text(group.firstElementChild))
            ? `Round ${text(group.firstElementChild)}`
            : '',
        },
      ];
    });
  }
  function tableAugments(doc, pageURL) {
    return [...doc.querySelectorAll('#ArenaWidget_table tr[data-table-row="data"]')].flatMap(
      (row) => {
        const cells = [...row.querySelectorAll('td')],
          image = row.querySelector('img');
        if (cells.length < 5 || !image) return [];
        const item = art(image, pageURL);
        const rateIndex = cells.findIndex(
          (cell, index) => index > 2 && percent(text(cell)) !== null,
        );
        const key =
          cells.map(text).find((value) => /^(?:augment|item)-\d+$/.test(value)) || item?.key;
        return item && rateIndex >= 0
          ? [
              {
                ...item,
                key,
                target: key,
                rarity: text(cells[1]),
                grade: text(cells[2]),
                pickRate: percent(text(cells[rateIndex])),
                games: text(cells[rateIndex + 1]),
              },
            ]
          : [];
      },
    );
  }
  function readRunes(section, pageURL) {
    if (!section) return null;
    const tabs = section.querySelector('[data-tab-url-value]');
    const content = tabs?.querySelector('[data-tab-target="content"]');
    const pages = [...section.querySelectorAll('[data-tab-key]')].map((tab) => {
      const icons = images(tab, pageURL),
        rates = [...tab.querySelectorAll('span')]
          .filter((n) => !n.children.length)
          .map((n) => percent(text(n)))
          .filter((n) => n !== null);
      return {
        key: tab.getAttribute('data-tab-key'),
        id: tab.id,
        items: icons,
        winRate: rates[0] ?? null,
        pickRate: rates[1] ?? null,
      };
    });
    const selected =
      section.querySelector('[data-tab-key].active')?.getAttribute('data-tab-key') || '0';
    const primary = [],
      secondary = [],
      shards = [];
    let tree = 0;
    for (const item of content ? images(content, pageURL) : []) {
      if (item.key.startsWith('tree-')) tree++;
      if (/^rune-5\d{3}$/.test(item.key)) shards.push(item);
      else (tree < 2 ? primary : secondary).push(item);
    }
    return { pages, selected, primary, secondary, shards };
  }
  function read(doc, href) {
    const source = sourceURL(href);
    if (!source) throw Error('Unsupported MetaSRC build URL');
    const header = doc.querySelector('#shell-header'),
      h1 = header?.querySelector('h1');
    const patch = text(h1).match(/Patch\s+([\d.]+)/)?.[1];
    const portrait = header?.querySelector('img[alt]');
    const widgets = [...doc.querySelectorAll('#build-section-content h2')].map((h) => ({
      name: text(h),
      node: h.closest('section'),
    }));
    const section = (name) =>
      widgets.find((w) => w.name === (name.startsWith(' ') ? `${portrait?.alt}${name}` : name))
        ?.node;
    if (!patch || !portrait || !widgets.length)
      throw Error('MetaSRC recommendations are still loading');
    if (slugFor(portrait.alt) !== source.slug) throw Error('MetaSRC returned a different champion');
    const arena = source.mode === 'arena',
      itemWidget = section(' Items');
    const mainItems = metricGroups(itemWidget, source.url, arena);
    const boots = metricGroups(section(' Boots'), source.url, arena);
    if (!mainItems.length && !boots.length) throw Error('MetaSRC supplied no item recommendations');
    const stats = [...header.querySelectorAll('[data-build-stats="full"] > span')]
      .map((n) => ({
        label: text(n.firstElementChild).replace(/:$/, ''),
        value: text(n.querySelector('strong')),
      }))
      .filter((n) => n.value);
    const styles = readStyles(header, source.url);
    const requestedBuild = new URL(source.url).searchParams.get('build') || 'all';
    if (
      styles.some((style) => style.key === requestedBuild) &&
      !styles.some((style) => style.key === requestedBuild && style.selected)
    ) {
      throw Error('MetaSRC is still loading the requested build');
    }
    const runeSection = section(' Runes');
    const skillSection = section(' Skill Order'),
      matrix = skillSection?.querySelector('.legacy-skill-order-table');
    const order = Array.from({ length: 18 }, (_, i) => {
      const keys = [...(matrix?.children || [])]
        .slice(1)
        .map((row) => text(row.children[i + 1]))
        .filter((s) => /^[QWER]$/.test(s));
      return keys.length === 1 ? keys[0] : null;
    });
    while (order.length && !order.at(-1)) order.pop();
    const skillText = text(skillSection);
    const priority =
      skillText.match(/Priority\s*([QWER])\s*>\s*([QWER])\s*>\s*([QWER])/)?.slice(1) || [];
    const skillRates = skillText.match(/(?:Win|Top 3)\s*%\s*([\d.]+%)\s*Pick\s*%\s*([\d.]+%)/);
    const filters = [
      ['region', 'Region', '#filters-region'],
      ['ranks', 'Rank', '[id^="range-selector-"]'],
      ['patch', 'Patch', '#filters-patch'],
    ].map(([id, label, selector]) => ({
      id,
      label,
      options: [...(header.querySelector(selector)?.querySelectorAll('[href]') || [])]
        .map((n) => ({ label: text(n), url: sourceURL(n.getAttribute('href'), source.url)?.url }))
        .filter((n) => n.label && n.url),
    }));
    if (!arena && source.mode === 'rift') {
      const roles = [...header.querySelectorAll('[href]')].flatMap((n) => {
        const target = sourceURL(n.getAttribute('href'), source.url);
        return target?.mode === 'rift' && target.slug === source.slug && target.role
          ? [
              {
                label: { top: 'Top', jungle: 'Jungle', mid: 'Mid', adc: 'ADC', support: 'Support' }[
                  target.role
                ],
                url: target.url,
              },
            ]
          : [];
      });
      const recommended = new URL(source.url);
      recommended.pathname = `/lol/champions/${source.slug}/build`;
      const currentRole = source.role || 'Recommended';
      const current = {
        label: source.role
          ? { top: 'Top', jungle: 'Jungle', mid: 'Mid', adc: 'ADC', support: 'Support' }[
              currentRole
            ]
          : currentRole,
        url: source.url,
      };
      filters.unshift({
        id: 'role',
        label: 'Role',
        options: [current, { label: 'Recommended', url: recommended.href }, ...roles].filter(
          (n, i, a) => a.findIndex((x) => x.url === n.url) === i,
        ),
      });
    }
    let augments = [],
      prismatics = [];
    if (arena) {
      try {
        const rows = JSON.parse(doc.querySelector('#ArenaWidget_row_data')?.textContent || '[]');
        if (Array.isArray(rows)) {
          const choices = rows.slice(0, 500).flatMap((row) => {
            const rarity = row.displays?.rarity,
              icon = assetURL(row.image);
            if (
              !['Silver', 'Gold', 'Prismatic', 'Item'].includes(rarity) ||
              !icon ||
              !/^(?:augment|item)-\d+$/.test(row.key)
            )
              return [];
            const tooltip = apiURL(
              doc
                .querySelector(
                  row.key.startsWith('item-')
                    ? '[data-tooltip-select^="#item-"]'
                    : '[data-tooltip-select^="#augment-"]',
                )
                ?.getAttribute('data-tooltip-url') || '',
              source.url,
            );
            return [
              {
                key: row.key,
                name: String(row.title || '').slice(0, 120),
                icon,
                rarity,
                grade: String(row.values?.tier || ''),
                pickRate: null,
                games: '',
                tooltip,
                target: row.key,
              },
            ];
          });
          augments = choices.filter((row) => row.key.startsWith('augment-'));
          prismatics = choices.filter((row) => row.key.startsWith('item-'));
        }
      } catch {
        /* A changed or incomplete source keeps the normal page visible. */
      }
      if (!augments.length) throw Error('MetaSRC augment recommendations are still loading');
      const supplied = new Map(tableAugments(doc, source.url).map((row) => [row.key, row]));
      augments = augments.map((row) => ({ ...row, ...supplied.get(row.key) }));
      const summary = new Map(
        metricGroups(section('Best Prismatic Items'), source.url, true).map((row) => [
          row.items[0]?.key,
          row,
        ]),
      );
      prismatics = prismatics.map((row) => ({
        ...row,
        pickRate: summary.get(row.key)?.pickRate ?? null,
        ...supplied.get(row.key),
      }));
    }
    const categories = arena
      ? [
          {
            id: 'prismatic',
            title: 'Prismatic Items',
            rows: metricGroups(section('Best Prismatic Items'), source.url, true),
          },
          ...widgets
            .filter((w) => /^Round \d+ Options$|^Elixirs$/.test(w.name))
            .map((w, i) => ({
              id: `option-${i}`,
              title: w.name,
              rows: metricGroups(w.node, source.url, true),
            })),
        ]
      : [
          {
            id: 'other',
            title: 'Other Items',
            rows: mainItems.filter((row) => row.items.length === 1),
          },
          ...widgets
            .filter((w) => /^Slot \d+ Item Options$|^Elixirs$/.test(w.name))
            .map((w, i) => ({
              id: `option-${i}`,
              title: w.name,
              rows: metricGroups(w.node, source.url, false),
            })),
        ];
    return {
      schema: SCHEMA,
      ...source,
      patch,
      champion: { name: portrait.alt, icon: assetURL(portrait.getAttribute('src')) },
      stats,
      filters,
      styles,
      augments,
      prismatics,
      augmentRows: tableAugments(doc, source.url),
      augmentEndpoint: augmentURL(
        doc
          .querySelector('[data-server-table-url-value]')
          ?.getAttribute('data-server-table-url-value') || '',
        source.url,
      ),
      skill: {
        abilities: ['Q', 'W', 'E', 'R'].map((key) => {
          const node = header.querySelector(
            `[data-tooltip-select="#skill-${key.toLowerCase()}"] img`,
          );
          return node ? art(node, source.url) : null;
        }),
        priority,
        order,
        winRate: skillRates ? percent(skillRates[1]) : null,
        pickRate: skillRates ? percent(skillRates[2]) : null,
      },
      runes: readRunes(runeSection, source.url),
      spells: metricGroups(section('Summoner Spells'), source.url, false),
      starting: metricGroups(section('Starting Items'), source.url, false),
      trinkets: metricGroups(section('Trinkets'), source.url, false),
      core: arena
        ? mainItems.filter((row) => row.label)
        : mainItems.filter((row) => row.items.length > 1),
      categories: categories.filter((c) => c.rows.length),
      boots,
      path: readPath(section('Item Build Order')?.querySelector('section'), source.url),
    };
  }
  function readCache(storage, url, at = Date.now()) {
    try {
      const raw = storage.getItem(CACHE_KEY) || '[]';
      if (raw.length > 1500000) return null;
      const rows = JSON.parse(raw);
      if (!Array.isArray(rows) || rows.length > 8) return null;
      const entry = rows.find((row) => row?.model?.url === url),
        model = entry?.model;
      if (
        model?.schema !== SCHEMA ||
        !sourceURL(url) ||
        !Number.isFinite(entry.at) ||
        at < entry.at ||
        at - entry.at > MAX_AGE ||
        !Array.isArray(model.augments) ||
        model.augments.length > 500 ||
        !Array.isArray(model.prismatics) ||
        model.prismatics.length > 100
      )
        return null;
      if (
        model.rateCheckedAt &&
        (!Number.isFinite(model.rateCheckedAt) ||
          model.rateCheckedAt > entry.at ||
          model.rateCheckedAt < entry.at - MAX_AGE)
      )
        return null;
      if (
        ![...model.augments, ...(model.prismatics || [])].every(
          (row) =>
            row &&
            /^(?:augment|item)-\d+$/.test(row.key) &&
            (row.pickRate === null ||
              (Number.isFinite(row.pickRate) && row.pickRate >= 0 && row.pickRate <= 100)) &&
            typeof row.games === 'string' &&
            row.games.length <= 30,
        )
      )
        return null;
      return model;
    } catch {
      return null;
    }
  }
  /** The saved rows, keeping other builds. A corrupt value is replaced, not kept forever: one
   *  half-written entry must not disable caching for every later guide. */
  function savedRows(storage) {
    try {
      const rows = JSON.parse(storage.getItem(CACHE_KEY) || '[]');
      return Array.isArray(rows) ? rows : [];
    } catch {
      return [];
    }
  }
  function saveCache(storage, model, at = Date.now()) {
    try {
      let rows = savedRows(storage);
      // Cache only source-reported augment rates, never executable content or markup.
      const cached = {
        schema: SCHEMA,
        url: model.url,
        patch: model.patch,
        rateCheckedAt: model.rateCheckedAt || 0,
        augments: model.augments.map(({ key, pickRate, games }) => ({ key, pickRate, games })),
        prismatics: model.prismatics.map(({ key, pickRate, games }) => ({ key, pickRate, games })),
      };
      rows = [
        { at, model: cached },
        ...rows.filter((r) => r?.model?.url !== model.url && at >= r?.at && at - r.at <= MAX_AGE),
      ].slice(0, 8);
      if (JSON.stringify(rows).length <= 1500000) storage.setItem(CACHE_KEY, JSON.stringify(rows));
    } catch {
      /* Storage restrictions do not prevent reading live recommendations. */
    }
  }
  function clearCache(storage, url) {
    try {
      storage.setItem(
        CACHE_KEY,
        JSON.stringify(savedRows(storage).filter((r) => r?.model?.url !== url)),
      );
    } catch {}
  }
  function install(options, environment = {}) {
    const win = environment.window || window,
      doc = environment.document || win.document;
    const currentHref = () =>
      typeof environment.url === 'function'
        ? environment.url()
        : environment.url || win.location.href;
    const source = sourceURL(currentHref());
    if (!source || doc.querySelector('fryiloco-guide')) return;
    // Paint the saved Oracle appearance before MetaSRC's page does, like guide_script later.
    function applyAppearance() {
      let saved = null;
      try {
        saved = JSON.parse(win.localStorage.getItem('umbra-guide-appearance') || 'null');
      } catch {}
      const t = saved?.t || options.appearance?.theme,
        c = saved?.c || options.appearance?.colour,
        page = doc.documentElement;
      if (!page || !/^[a-z][a-z0-9-]*$/.test(t || '') || !/^#[0-9a-f]{6}$/i.test(c || '')) return;
      const mix = (a) =>
        '#' +
        c
          .slice(1)
          .match(/../g)
          .map((x) => Math.round(parseInt(x, 16) * (1 - a) + 255 * a).toString(16).padStart(2, '0'))
          .join('');
      const ruby = c.toLowerCase() === '#ae1236';
      page.dataset.theme = t;
      page.style.setProperty('--accent', c);
      page.style.setProperty('--silver', ruby ? '#cabfb1' : mix(0.6));
      page.style.setProperty('--shine', ruby ? '#fff3e7' : mix(0.92));
    }
    const navigate = environment.navigate || ((url) => win.location.assign(url));
    let storage;
    try {
      storage = environment.storage || win.localStorage;
    } catch {
      storage = null;
    }
    const abort = new AbortController();
    let itemNames = false;
    try {
      itemNames = storage?.getItem('fryiloco-guide-item-names-v1') === 'true';
    } catch {}
    let contextAbort = new AbortController();
    async function fetchDocument(url, signal = abort.signal) {
      const target = apiURL(url, currentHref());
      if (!target) throw Error('Unsupported MetaSRC data URL');
      if (environment.fetchDocument) return environment.fetchDocument(target, { signal });
      const response = await win.fetch(target, {
        credentials: 'same-origin',
        redirect: 'error',
        signal: AbortSignal.any([abort.signal, signal, AbortSignal.timeout(10000)]),
      });
      if (!response.ok) throw Error(`MetaSRC request failed (${response.status})`);
      const reader = response.body?.getReader();
      if (!reader) throw Error('Empty MetaSRC response');
      const chunks = [];
      let bytes = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          bytes += value.length;
          if (bytes > 2000000) throw Error('MetaSRC response was too large');
          chunks.push(value);
        }
      } finally {
        await reader.cancel().catch(() => {});
      }
      const data = new Uint8Array(bytes);
      let offset = 0;
      for (const chunk of chunks) {
        data.set(chunk, offset);
        offset += chunk.length;
      }
      return new win.DOMParser().parseFromString(new TextDecoder().decode(data), 'text/html');
    }
    let host,
      root,
      model,
      sourceStyle,
      updating = false,
      observer,
      challengeObserver,
      updateTimer,
      pending;
    // Models already read from the source in this document, by build URL. Switching back to a
    // build shows it at once while the source catches up. Kept in memory only.
    const loaded = new Map();
    const challenged = () => doc.documentElement?.classList.contains('dynamic-challenge');
    function markChallengePath() {
      const widget = doc.getElementById('hyper-dynamic-turnstile');
      for (const node of doc.querySelectorAll('[data-umbra-challenge-path]'))
        if (!widget || !node.contains(widget)) node.removeAttribute('data-umbra-challenge-path');
      for (let node = widget?.parentElement; node && node !== doc.body; node = node.parentElement)
        node.setAttribute('data-umbra-challenge-path', '');
    }
    function changeFailed(message) {
      endChange();
      // A saved build may be on screen while the source still shows the previous one.
      update();
      notice(message);
    }
    const sourceHiding =
      'html{overflow:hidden!important}body{height:100dvh!important;min-height:0!important;margin:0!important;overflow:hidden!important}body>:not(fryiloco-guide):not([data-umbra-challenge-path]){display:none!important}' +
      // MetaSRC asks for a Cloudflare Turnstile check before the first build change on a page.
      // Keep only the path to its widget laid out (invisibly) and show the widget above the
      // guide while MetaSRC marks the page as challenged, so the player can complete it.
      '[data-umbra-challenge-path]{visibility:hidden!important;z-index:2147483647!important}[data-umbra-challenge-path]>:not([data-umbra-challenge-path]):not(#hyper-dynamic-turnstile){display:none!important}' +
      'html.dynamic-challenge #hyper-dynamic-turnstile{visibility:visible!important;position:fixed!important;inset:0!important;z-index:2147483647!important;background:rgba(4,5,9,.72)!important;pointer-events:auto!important}';
    let needle = '',
      prismaticNeedle = '',
      tooltipTarget = null,
      tipTimer,
      restoringTipFocus = false;
    const tips = new Map(),
      tipRequests = new Map();
    const q = (selector) => root.querySelector(selector);
    const clearTip = (restoreFocus = false) => {
      clearTimeout(tipTimer);
      const target = tooltipTarget;
      target?.removeAttribute('aria-describedby');
      if (root) q('#item-tooltip').hidden = true;
      tooltipTarget = null;
      if (restoreFocus && target?.isConnected) {
        restoringTipFocus = true;
        target.focus({ preventScroll: true });
        restoringTipFocus = false;
      }
      if (root?.querySelector('[data-pending-order]'))
        setTimeout(() => {
          if (abort.signal.aborted) return;
          for (const list of root.querySelectorAll('[data-pending-order]')) {
            if (list.id === 'meta-prismatic-items') renderPrismatics(true);
            else renderAugments(true);
          }
        }, 0);
    };
    function image(item, focusable = true) {
      if (!item) return '';
      const key = `${item.key}|${item.tooltip || item.icon}`;
      tips.set(key, item);
      const icon = assetURL(item.icon),
        initials = item.name
          .split(/\s+/)
          .slice(0, 2)
          .map((word) => word[0])
          .join('');
      return `<span class="meta-art${icon ? '' : ' art-failed'}" data-tip="${escape(key)}" tabindex="${focusable ? 0 : -1}" aria-keyshortcuts="F2" aria-label="${escape(item.name)}${item.quantity > 1 ? `, quantity ${item.quantity}` : ''}">${icon ? `<img src="${escape(icon)}" alt="" loading="lazy">` : ''}<span class="art-fallback" aria-hidden="true">${escape(initials)}</span>${item.quantity > 1 ? `<small class="meta-stack-count">${item.quantity}</small>` : ''}</span>`;
    }

    const mark = (name) =>
      `<svg class="section-icon" aria-hidden="true"><use href="#mark-${name}"/></svg>`;
    // count: number of tiles in the card. Cards in one row grow in proportion
    // to it, so every tile in that row gets the same width.
    function card(title, content, extra = '', count = 0) {
      const icon = /Skill/.test(title)
        ? 'skills'
        : /Rune/.test(title)
          ? 'augments'
          : /Boot/.test(title)
            ? 'boots'
            : /Core|Recommended/.test(title)
              ? 'core'
              : /Order|Later/.test(title)
                ? 'other'
                : 'items';
      return `<section class="meta-card ${extra}" aria-label="${escape(title)}"${count ? ` style="--n:${Math.max(1, Math.round(count))}"` : ''}><h3>${mark(icon)}${escape(title)}</h3>${content}</section>`;
    }
    // A long compound name gets a soft hyphen before its last part, so a tile too
    // narrow for it shows "Morello-nomicon" rather than a word cut at any letter.
    const compoundTail =
      /^(.{4,}?)(nomicon|breaker|thirster|letter|blade|flame|heart|plate|mail|cannon|carver|caster|eater|surge|sword|spike|arrows|bloom|belt|song|steel|maker|sleigh|guard|ward|fire|storm|shield|light|born)('s)?$/i;
    const breakable = (name) =>
      String(name).replace(/[A-Za-z']{10,}/g, (word) => word.replace(compoundTail, '$1­$2$3'));
    // Label and value are separate cells of a two-column grid so they line up;
    // their wrapper only counts where a card sets rates on one line.
    const rate = (label, value) =>
      `<span class="meta-rate"><span class="meta-rate-label">${label}</span><strong>${value}</strong></span>`;
    function tiles(rows, extra = '') {
      return `<div class="meta-choices ${extra}">${rows.map((row) => `<div class="meta-choice"><div class="meta-choice-icons">${row.items.map((item) => `<span class="meta-named-item">${image(item)}<span class="meta-item-label">${escape(breakable(item.name))}</span></span>`).join('')}</div><div class="meta-choice-rates">${Number.isFinite(row.winRate) ? rate('Win', format(row.winRate)) : row.grade ? rate('Tier', escape(row.grade)) : ''}${rate(row.metric === 'pick' ? 'Pick' : 'Buy', format(row.pickRate))}${row.games ? `<small>${escape(row.games)} games</small>` : ''}</div></div>`).join('') || '<p class="empty">Not reported for this mode.</p>'}</div>`;
    }
    function pathView() {
      // Each step carries its leading arrow so a wrapped line starts with one.
      return `<div class="meta-purchase-path" tabindex="0" role="region" aria-label="Item build order">${model.path.length ? `<div class="meta-purchase-line">${model.path.map((item, index) => `<span class="meta-step">${index ? '<span class="meta-path-arrow" aria-hidden="true">›</span>' : ''}<span class="meta-purchase${item.cost ? ' completed' : ''}">${image(item)}<span class="meta-item-label">${escape(breakable(item.name))}</span>${item.cost ? `<small><span aria-label="Gold">◈</span> ${escape(item.cost)}</small>` : ''}</span></span>`).join('')}</div>` : '<p class="empty">No purchase path reported.</p>'}</div>`;
    }
    function skillView() {
      const skill = model.skill;
      return `<div class="meta-skill-scroll"><table class="meta-skill-matrix" aria-label="Reported skill upgrades"><thead><tr><th scope="col"><span class="sr-only">Ability</span></th>${Array.from({ length: 18 }, (_, i) => `<th scope="col" class="meta-level"><span class="sr-only">Level </span>${i + 1}</th>`).join('')}</tr></thead><tbody>${['Q', 'W', 'E', 'R'].map((key, row) => `<tr><th scope="row" class="meta-ability"><span class="sr-only">${key} </span>${skill.abilities[row] ? image(skill.abilities[row]) : key}</th>${Array.from({ length: 18 }, (_, i) => `<td class="meta-skill-cell ${skill.order[i] === key ? `learned skill-${key}` : ''}">${skill.order[i] === key ? `${key}<span class="sr-only">, upgrade</span>` : '<span class="sr-only">No upgrade</span>'}</td>`).join('')}</tr>`).join('')}</tbody></table></div><div class="meta-skill-summary"><span>Priority <b>${skill.priority.join(' › ') || '—'}</b></span><span>Win <strong>${format(skill.winRate)}</strong></span><span>Pick <strong>${format(skill.pickRate)}</strong></span></div>`;
    }
    function runeView() {
      const runes = model.runes;
      if (!runes) return '<p class="empty">Runes are not available.</p>';
      return `<div class="meta-rune-pages" role="group" aria-label="Rune alternatives">${runes.pages.map((page) => `<button type="button" data-rune-page="${escape(page.key)}" aria-pressed="${page.key === runes.selected}" aria-label="${escape(page.items.map((i) => i.name).join(' / '))}, ${format(page.winRate)} win, ${format(page.pickRate)} pick" title="${escape(page.items.map((i) => i.name).join(' / '))}"><span>${page.items.map((item) => image(item, false)).join('')}</span><small>${format(page.winRate)} <em>win</em></small><small>${format(page.pickRate)} <em>pick</em></small></button>`).join('')}</div><div class="meta-rune-line primary" aria-label="Primary runes">${runes.primary.map((item) => image(item)).join('')}</div><div class="meta-rune-line secondary" aria-label="Secondary runes and stat shards">${runes.secondary.map((item) => image(item)).join('')}<span class="meta-rune-divider"></span>${runes.shards.map((item) => image(item)).join('')}</div>`;
    }
    function renderStandard() {
      const other = model.categories.find((c) => c.id === 'other')?.rows || [];
      const trinketIds = new Set(['item-3340', 'item-3363', 'item-3364']);
      const trinkets = model.trinkets.length
        ? model.trinkets
        : [...other, ...model.starting]
            .filter((row) => row.items.length === 1 && trinketIds.has(row.items[0].key))
            .filter(
              (row, index, rows) =>
                rows.findIndex((r) => r.items[0].key === row.items[0].key) === index,
            );
      const options = model.categories.filter((c) => c.id !== 'other');
      const starting = model.starting.filter(
        (row) => !trinkets.some((t) => t.items[0].key === row.items[0].key),
      );
      const consumables = new Set(
        model.categories
          .filter((c) => c.title === 'Elixirs')
          .flatMap((c) => c.rows.flatMap((row) => row.items.map((item) => item.key))),
      );
      const laterItems = other.filter(
        (row) =>
          !trinkets.some((t) => t.items[0].key === row.items[0].key) &&
          !consumables.has(row.items[0].key),
      );
      const spellPairs = model.spells.filter((row) => row.items.length > 1);
      const spellAlternatives = model.spells.filter((row) => row.items.length === 1);
      const spells = `<div class="meta-spell-groups">${spellPairs.length ? `<section class="meta-spell-pair"><h4>Recommended pair</h4>${tiles(spellPairs)}</section>` : ''}${spellAlternatives.length ? `<section style="--n:${spellAlternatives.length}"><h4>Alternatives</h4>${tiles(spellAlternatives)}</section>` : ''}</div>`;
      // Three rows of plain cards: setup, build, then per-slot options.
      q('#guide-content').innerHTML =
        `<div class="meta-standard"><div class="meta-row meta-row-top">${card('Runes', runeView(), 'meta-runes')}${card('Skill Order', skillView(), 'meta-skills')}<div class="meta-essentials">${card('Summoner Spells', spells, 'meta-spells')}${card('Starting Items', tiles(starting), 'meta-starting')}</div></div><div class="meta-row meta-row-build">${card('Core Build', tiles(model.core), 'meta-core')}${laterItems.length ? card('Later Items', tiles(laterItems), 'meta-later') : ''}${card('Item Build Order', pathView(), 'meta-path')}</div><div class="meta-row meta-options">${options.map((c) => card(c.title, tiles(c.rows), 'meta-option', c.rows.length)).join('')}${card('Boots', tiles(model.boots), 'meta-option meta-boots', model.boots.length)}${trinkets.length ? card('Trinkets', tiles(trinkets), 'meta-option', trinkets.length) : ''}</div></div>`;
    }
    function arenaTemplate() {
      // Left: the same three augment lists as Mayhem. Right: item cards stacked
      // in one column, with the prismatic list taking the remaining height.
      const panel = q('.augments-panel');
      q('#guide-content').innerHTML =
        '<div class="meta-arena-layout"><div id="meta-augment-slot"></div><div class="meta-arena-side"><div class="panel-heading"><h2>' +
        mark('items') +
        'Items</h2><span class="heading-note">Buy rate · games</span></div><section class="meta-card meta-arena-rounds" aria-label="Recommended items by round"><h3>' +
        mark('core') +
        'Recommended by round</h3><div id="meta-recommended-rounds" class="meta-recommended-rounds"></div></section><div id="meta-round-options" class="meta-round-options"></div><section class="meta-card meta-prismatic-sheet" aria-label="All prismatic items"><h3>' +
        mark('items') +
        'Prismatic Items <span id="meta-prismatic-count" class="count" aria-live="polite"></span><label class="meta-prismatic-search"><input type="search" id="meta-prismatic-search" aria-label="Search prismatic items" placeholder="Search items…"></label><span class="meta-strip-note">Tier · pick rate</span><span class="meta-prismatic-sort" role="group" aria-label="Sort prismatic items"><button type="button" data-prismatic-sort="tier" aria-label="Sort by tier" title="Sort by tier">Tier</button><button type="button" data-prismatic-sort="pickRate" aria-label="Sort by pick rate" title="Sort by pick rate">Pick</button></span></h3><div id="meta-prismatic-items" tabindex="0" role="region" aria-label="All prismatic item recommendations"></div></section></div></div>';
      q('#meta-augment-slot').replaceWith(panel);
    }
    function renderEquipment() {
      const choices = model.categories
        .filter((c) => c.id !== 'prismatic')
        .sort(
          (a, b) =>
            Number(a.title.match(/\d+/)?.[0] || 100) - Number(b.title.match(/\d+/)?.[0] || 100),
        );
      q('#meta-recommended-rounds').innerHTML =
        model.core
          .map(
            (row) =>
              `<div class="meta-recommended-round"><span>${escape(row.label)}</span>${tiles([row])}</div>`,
          )
          .join('') || '<p class="empty">No round recommendations reported.</p>';
      q('#meta-round-options').innerHTML =
        choices.map((c) => card(c.title, tiles(c.rows), 'meta-option', c.rows.length)).join('') +
        card('Boots', tiles(model.boots), 'meta-option', model.boots.length);
    }
    let prismaticSort = 'tier';
    // Rate pagination must not replace or move an item being read. Reconcile
    // by source identity, updating only metrics on existing rows.
    function syncRecommendations(list, rows, prismatic, background) {
      if (!list.dataset.reconciled) {
        list.dataset.reconciled = 'true';
        const settle = () => {
          if (!abort.signal.aborted && list.dataset.pendingOrder)
            (prismatic ? renderPrismatics : renderAugments)(true);
        };
        list.addEventListener('mouseleave', settle);
        list.addEventListener('focusout', () => setTimeout(settle, 0));
      }
      const inspecting =
        list.contains(root.activeElement) ||
        (tooltipTarget && list.contains(tooltipTarget)) ||
        list.matches(':hover');
      const existing = new Map(
        [...list.querySelectorAll('[data-entry]')].map((n) => [n.dataset.entry, n]),
      );
      if (background && inspecting) {
        const positions = new Map([...existing.keys()].map((key, i) => [key, i]));
        rows = [...rows].sort(
          (a, b) => (positions.get(a.key) ?? Infinity) - (positions.get(b.key) ?? Infinity),
        );
        list.dataset.pendingOrder = 'true';
      } else delete list.dataset.pendingOrder;
      const wanted = new Set(rows.map((row) => row.key));
      for (const [key, node] of existing)
        if (!wanted.has(key)) {
          if (tooltipTarget && node.contains(tooltipTarget)) clearTip();
          node.remove();
        }
      list.querySelector('.empty')?.remove();
      let cursor = list.firstElementChild;
      for (const item of rows) {
        let node = existing.get(item.key);
        if (!node) {
          node = doc.createElement('article');
          node.dataset.entry = item.key;
          node.className = prismatic ? 'meta-prismatic-item' : 'augment-row';
          node.innerHTML = prismatic
            ? `${image(item)}<span class="meta-item-label"></span><span class="meta-prismatic-grade grade"></span><small class="recommendation-rate"></small>`
            : `<span class="augment-art">${image(item)}</span><span class="entry-name"></span><div class="augment-metrics"><span class="grade"></span><span class="augment-pick recommendation-rate" title="Pick rate"></span></div>`;
        }
        node.setAttribute('aria-label', item.name);
        const target = node.querySelector('[data-tip]');
        const key = `${item.key}|${item.tooltip || item.icon}`;
        tips.set(key, item);
        if (target) {
          target.dataset.tip = key;
          target.setAttribute('aria-label', item.name);
          const src = assetURL(item.icon);
          let picture = target.querySelector('img');
          if (src && picture?.getAttribute('src') !== src) {
            if (!picture) {
              picture = doc.createElement('img');
              picture.alt = '';
              picture.loading = 'lazy';
              target.prepend(picture);
            }
            picture.src = src;
            picture.classList.remove('missing-art');
            target.classList.remove('art-failed');
          } else if (!src) {
            picture?.remove();
            target.classList.add('art-failed');
          }
          target.querySelector('.art-fallback').textContent = item.name
            .split(/\s+/)
            .slice(0, 2)
            .map((word) => word[0])
            .join('');
        }
        const name = node.querySelector(prismatic ? '.meta-item-label' : '.entry-name');
        if (name) name.textContent = prismatic ? breakable(item.name) : item.name;
        const grade = node.querySelector(prismatic ? '.meta-prismatic-grade' : '.grade');
        grade.textContent = item.grade || '—';
        // S+ and S share the gold badge; the letter decides the colour.
        grade.className = `${prismatic ? 'meta-prismatic-grade ' : ''}grade${/^[SABCD]/.test(item.grade || '') ? ` grade-${item.grade[0]}` : ''}`;
        node.querySelector('.recommendation-rate').textContent = format(item.pickRate);
        if (node !== cursor) list.insertBefore(node, cursor);
        cursor = node.nextElementSibling;
      }
      if (!rows.length) {
        const empty = doc.createElement('p');
        empty.className = 'empty';
        empty.textContent = 'No matching recommendations.';
        list.append(empty);
      }
    }
    function renderPrismatics(background = false) {
      const list = q('#meta-prismatic-items'),
        scroll = list.scrollTop;
      const rows = model.prismatics.filter((item) =>
        item.name.toLowerCase().includes(prismaticNeedle.trim().toLowerCase()),
      );
      q('#meta-prismatic-count').textContent =
        rows.length === model.prismatics.length
          ? String(rows.length)
          : `${rows.length} of ${model.prismatics.length}`;
      syncRecommendations(list, sorted(rows, prismaticSort), true, background);
      list.scrollTop = scroll;
      for (const button of root.querySelectorAll('[data-prismatic-sort]'))
        button.setAttribute('aria-pressed', String(button.dataset.prismaticSort === prismaticSort));
    }
    function renderAugments(background = false) {
      const rows = sorted(
        model.augments.filter((a) => a.name.toLowerCase().includes(needle.trim().toLowerCase())),
        'tier',
      );
      q('#augment-count').textContent = String(rows.length);
      for (const rarity of ['Silver', 'Gold', 'Prismatic']) {
        const key = rarity.toLowerCase(),
          list = q(`#${key}-rows`),
          scroll = list.scrollTop,
          filtered = rows.filter((r) => r.rarity === rarity);
        q(`#${key}-count`).textContent = String(filtered.length);
        syncRecommendations(list, filtered, false, background);
        list.scrollTop = scroll;
      }
    }

    // Before MetaSRC's data has arrived, the banner already names the champion and mode.
    function placeholder() {
      if (!root || model) return;
      const name = (options.champions || []).find((c) => c.slug === source.slug)?.name || '';
      const modeName = { arena: 'ARENA', aram: 'ARAM', rift: 'RANKED' }[source.mode];
      host.className = `meta-guide mode-${source.mode}${itemNames ? ' named-items' : ''}`;
      q('#meta-item-names').setAttribute('aria-pressed', String(itemNames));
      q('#champion-name').textContent = name;
      q('.meta-portrait').classList.add('art-failed');
      q('.meta-portrait .art-fallback').textContent = name
        .split(/\s+/)
        .map((word) => word[0] || '')
        .slice(0, 2)
        .join('');
      q('#meta-mode-name').textContent = modeName;
      q('#meta-mode-name').nextSibling.textContent =
        source.mode === 'arena' ? ' · AUGMENTS · SKILLS · ITEMS' : ' · RUNES · SKILLS · ITEMS';
      q('#champion-select').disabled = true;
      q('#meta-build-styles').hidden = true;
      q('#loading').hidden = false;
      if (name) doc.title = `${name} · ${modeName} · Oracle`;
    }
    // Shows MetaSRC's own page when it is not a readable build page, such as a full-page human
    // check or a changed layout, instead of leaving the player on a loading view.
    function uncover() {
      if (model || !host) return;
      host.remove();
      sourceStyle?.remove();
      host = root = sourceStyle = null;
    }
    function render() {
      if (!root) return;
      if (!model) return placeholder();
      clearTip();
      tips.clear();
      const modeName = { arena: 'ARENA', aram: 'ARAM', rift: 'RANKED' }[model.mode];
      host.className = `meta-guide mode-${model.mode}${itemNames ? ' named-items' : ''}`;
      q('#meta-item-names').setAttribute('aria-pressed', String(itemNames));
      q('#champion-name').textContent = model.champion.name;
      q('#champion-icon').src = model.champion.icon;
      q('#champion-icon').classList.remove('missing-art');
      q('.meta-portrait').classList.remove('art-failed');
      q('.meta-portrait .art-fallback').textContent = model.champion.name
        .split(/\s+/)
        .map((word) => word[0])
        .slice(0, 2)
        .join('');
      q('#meta-mode-name').textContent = modeName;
      q('#meta-mode-name').nextSibling.textContent =
        model.mode === 'arena' ? ' · AUGMENTS · SKILLS · ITEMS' : ' · RUNES · SKILLS · ITEMS';
      q('#data-date').textContent = `Patch ${model.patch}`;
      doc.title = `${model.champion.name} · ${modeName} · Oracle`;
      const wanted =
        model.mode === 'arena' ? ['Top 3 %', 'Pick %', 'Placement'] : ['Win %', 'Pick %', 'Tier'];
      q('#champion-stats').innerHTML = wanted
        .map((key) => model.stats.find((s) => s.label === key))
        .filter(Boolean)
        .map(
          (s) =>
            `<div class="stat"><strong>${escape(s.value)}</strong><span>${escape({ 'Top 3 %': 'Top 3 rate', 'Pick %': 'Pick rate', 'Win %': 'Win rate', Placement: 'Avg. placement' }[s.label] || s.label)}</span></div>`,
        )
        .join('');
      const select = q('#champion-select');
      select.innerHTML = (options.champions || [])
        .map(
          (c) =>
            `<option value="${escape(c.slug)}"${c.slug === model.slug ? ' selected' : ''}>${escape(c.name)}</option>`,
        )
        .join('');
      select.disabled = !select.options.length;
      q('#meta-build-styles').innerHTML = model.styles
        .map(
          (style) =>
            `<button type="button" data-build-style="${escape(style.url)}" aria-pressed="${style.selected}" aria-label="${escape(style.label)} build" title="${escape(style.label)} build">${style.items
              .slice(-1)
              .map((item) => image(item, false))
              .join(
                '',
              )}${style.key === 'all' ? '<span aria-hidden="true">★</span>' : `<small>${escape(style.label)}</small>`}</button>`,
        )
        .join('');
      q('#meta-build-styles').hidden = !model.styles.length;
      if (model.mode === 'arena') {
        q('#meta-arena-skills').innerHTML =
          `<span class="skills-label">${mark('skills')}Skills</span><strong class="skill-priority">${model.skill.priority.join(' › ')}</strong><div class="skill-upgrades" tabindex="0" role="region" aria-label="Reported skill upgrades">${model.skill.order.map((key, index) => `<div class="skill-upgrade" aria-label="Level ${index + 1}, ${key || 'not reported'}"><small>${index + 1}</small><span class="skill-key skill-${key || 'unknown'}">${key || '—'}</span></div>`).join('')}</div>`;
        if (!q('.meta-arena-layout')) arenaTemplate();
        q('#meta-prismatic-search').value = prismaticNeedle;
        renderAugments();
        renderEquipment();
        renderPrismatics();
      } else renderStandard();
      q('#meta-arena-skills').hidden = model.mode !== 'arena';
      q('#loading').hidden = true;
      q('#error').hidden = true;
      q('#guide-content').hidden = false;
      q('footer').innerHTML =
        `<span>MetaSRC · Patch ${escape(model.patch)} · ${escape(model.stats.find((s) => s.label === 'Games')?.value || '')} games</span><span>${model.mode === 'arena' ? 'Champion/skills: top 3 · Items: buy rate · Augments/prismatics: tier / pick rate' : 'Items: win / buy · Runes & spells: win / pick'}</span>`;
    }
    function notice(value, force = false) {
      if (!root) return;
      if (pending && !force) return;
      q('#notice').textContent = value;
      q('#notice').hidden = !value;
    }
    function busyControls() {
      if (!root) return;
      for (const button of root.querySelectorAll('[data-build-style],[data-rune-page]')) {
        button.setAttribute(
          'aria-busy',
          String(
            !!pending &&
              (button.dataset.buildStyle === pending.key ||
                button.dataset.runePage === pending.key),
          ),
        );
      }
    }
    function endChange(error = '') {
      if (pending) clearTimeout(pending.timer);
      pending = null;
      busyControls();
      notice(error);
    }
    function beginChange(kind, key, label, node) {
      if (pending?.kind === kind && pending.key === key) return false;
      if (pending) clearTimeout(pending.timer);
      const request = {
        kind,
        key,
        node,
        label,
        changed: false,
        content: kind === 'rune' ? doc.querySelector('[data-tab-target="content"]') : null,
      };
      const expire = () => {
        if (pending !== request) return;
        // Waiting for the player to complete MetaSRC's check is not a failed load.
        if (challenged()) {
          request.timer = setTimeout(expire, 1000);
          return;
        }
        changeFailed(
          `${label} did not finish loading. Previous recommendations are still shown; refresh or choose another build.`,
        );
      };
      request.timer = setTimeout(expire, environment.changeTimeoutMs || 12000);
      pending = request;
      clearTip();
      busyControls();
      notice(`Loading ${label}…`, true);
      return true;
    }
    async function tooltip(target) {
      const item = tips.get(target.dataset.tip);
      if (!item || restoringTipFocus) return;
      clearTimeout(tipTimer);
      if (tooltipTarget === target) return;
      clearTip();
      tooltipTarget = target;
      const tip = q('#item-tooltip');
      tip.textContent = item.name;
      tip.hidden = false;
      target.setAttribute('aria-describedby', 'item-tooltip');
      const position = () => {
        const box = target.getBoundingClientRect(),
          w = tip.offsetWidth;
        tip.style.left = `${Math.max(8, Math.min(box.left, host.clientWidth - w - 8))}px`;
        tip.style.top = `${Math.max(8, Math.min(box.bottom + 8, win.innerHeight - tip.offsetHeight - 8))}px`;
      };
      position();
      if (!item.tooltip) return;
      try {
        if (!tipRequests.has(item.tooltip))
          tipRequests.set(item.tooltip, fetchDocument(item.tooltip));
        const data = await tipRequests.get(item.tooltip);
        if (tooltipTarget !== target || !target.isConnected) return;
        const content = data.getElementById(item.target);
        if (content) {
          const copy = content.cloneNode(true);
          for (const node of copy.querySelectorAll('script,style')) node.remove();
          for (const node of copy.querySelectorAll('br'))
            node.replaceWith(data.createTextNode('\n'));
          for (const node of copy.querySelectorAll('p,div,li'))
            node.append(data.createTextNode('\n'));
          const description = (copy.textContent || '')
            .replace(/[ \t]+/g, ' ')
            .replace(/ *\n */g, '\n')
            .replace(/\n{3,}/g, '\n\n')
            .trim();
          tip.textContent =
            description.length > 2400 ? `${description.slice(0, 2400)}…` : description;
        }
        position();
      } catch {
        tipRequests.delete(item.tooltip);
      }
    }
    function mount() {
      host = doc.createElement('fryiloco-guide');
      // The host's base stylesheet must inherit the saved asset colour from the document.
      for (const name of ['silver','accent','shine']) host.style.setProperty('--'+name,'inherit');
      root = host.attachShadow({ mode: 'open' });
      sourceStyle = doc.createElement('style');
      sourceStyle.textContent = sourceHiding;
      doc.head.append(sourceStyle);
      const body = options.template.match(/<body[^>]*>([\s\S]*?)<\/body>/i)?.[1];
      if (!body) throw Error('The guide template is unavailable');
      const style = options.style.replace(/:root((?:\[data-(?:theme|glass)="[^"]+"\])+)/g, ':host-context($1)').replace(/:root|\bhtml\b|\bbody\b/g, ':host');
      root.innerHTML = `<style>${style}\n:host{box-sizing:border-box;position:fixed!important;inset:0!important;z-index:2147483646!important;isolation:isolate;text-align:left;}[hidden]{display:none!important}</style>${body.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')}`;

      // Same banner as the Mayhem page: champion | skills | rates | actions.
      q('.toolbar').outerHTML =
        `<header class="toolbar meta-banner"><div class="meta-identity champion-picker" title="Choose a champion"><span class="meta-portrait"><img id="champion-icon" alt=""><span class="art-fallback" aria-hidden="true">?</span></span><div class="meta-title"><h1><span id="champion-name"></span> <span class="meta-title-gold">BUILD GUIDE</span></h1><small><span id="meta-mode-name"></span> · RUNES · SKILLS · ITEMS</small><span class="champion-picker-cue" aria-hidden="true">Change champion <span>⌄</span></span></div><select id="champion-select" aria-label="Choose a champion"></select></div><div id="meta-arena-skills" class="skills-strip" hidden></div><div id="champion-stats" class="champion-stats"></div><div class="toolbar-actions"><span class="date-badge"><svg class="section-icon" aria-hidden="true"><use href="#mark-calendar"/></svg><span id="data-date"></span></span><button id="refresh" class="button refresh-button" type="button" title="Refresh recommendations" aria-label="Refresh recommendations">↻</button></div></header>`;
      q('.champion-hero')?.remove();
      const bar = doc.createElement('div');
      bar.className = 'meta-build-toolbar';
      bar.innerHTML =
        '<nav id="meta-build-styles" aria-label="Build type"></nav><button type="button" id="meta-item-names" aria-pressed="true">Item names</button>';
      q('.toolbar').after(bar);
      q('.heading-note').textContent = 'Pick rate · tier';
      q('.heading-note').title = 'MetaSRC augment tiers and pick rates';
      const tip = q('#item-tooltip');
      tip.tabIndex = 0;
      tip.setAttribute('aria-live', 'polite');
      tip.setAttribute('aria-atomic', 'true');
      root.addEventListener('click', (event) => {
        const button = event.target.closest('button');
        if (!button) return;
        if (button.id === 'meta-item-names') {
          clearTip();
          itemNames = !itemNames;
          host.classList.toggle('named-items', itemNames);
          button.setAttribute('aria-pressed', String(itemNames));
          try {
            storage?.setItem('fryiloco-guide-item-names-v1', String(itemNames));
          } catch {}
        } else if (button.id === 'refresh' || button.id === 'retry') {
          if (model) clearCache(storage, model.url);
          win.location.reload();
        } else if (!model) {
          return;
        } else if (button.dataset.buildStyle) {
          const style = model.styles.find((row) => row.url === button.dataset.buildStyle);
          if (!style || (!pending && style.selected && style.url === model.url)) return;
          const header = doc.querySelector('#shell-header');
          const liveStyle =
            header && readStyles(header, currentHref()).find((row) => row.url === style.url);
          const node = doc.getElementById(liveStyle?.id || style.id);
          if (!beginChange('build', style.url, `${style.label} build`, node)) return;
          const ready = loaded.get(style.url);
          if (ready && ready.patch === model.patch) {
            model = ready;
            render();
            busyControls();
            notice('', true);
            root.querySelector(`[data-build-style="${CSS.escape(style.url)}"]`)?.focus();
          }
          // Use the site's real HTMX link. Full document GETs initially contain
          // Recommended data and can mount before the source finishes hydrating.
          try {
            if (
              !environment.navigate &&
              node &&
              sourceURL(node.getAttribute('href'), currentHref())?.url === style.url
            )
              node.click();
            else navigate(style.url);
          } catch {
            changeFailed(
              'The selected build could not be loaded. Previous recommendations are still shown.',
            );
          }
        } else if (button.dataset.runePage) {
          const page = model.runes?.pages.find((p) => p.key === button.dataset.runePage),
            node = page && doc.getElementById(page.id);
          if (node) {
            if (
              (!pending && page.key === model.runes.selected) ||
              !beginChange('rune', page.key, 'the selected rune page', node)
            )
              return;
            try {
              node.click();
            } catch {
              endChange(
                'The selected rune page could not be loaded. Previous runes are still shown.',
              );
            }
          }
        } else if (['tier', 'pickRate'].includes(button.dataset.prismaticSort)) {
          prismaticSort = button.dataset.prismaticSort;
          clearTip();
          renderPrismatics();
          q(`[data-prismatic-sort="${prismaticSort}"]`).focus();
        }
      });
      root.addEventListener('change', (event) => {
        if (
          event.target.id !== 'champion-select' ||
          !model ||
          !(options.champions || []).some((c) => c.slug === event.target.value)
        )
          return;
        const next = new URL(model.url);
        next.pathname = `/lol/${model.mode === 'rift' ? '' : `${model.mode}/`}champions/${event.target.value}${model.role ? `/${model.role}` : ''}/build`;
        next.searchParams.delete('build');
        navigate(next.href);
      });
      root.addEventListener('input', (event) => {
        if (event.target.id === 'augment-search') {
          needle = event.target.value;
          renderAugments();
        } else if (event.target.id === 'meta-prismatic-search') {
          prismaticNeedle = event.target.value;
          renderPrismatics();
        }
      });
      for (const event of ['mouseover', 'focusin'])
        root.addEventListener(event, (e) => {
          if (tip.contains(e.target)) {
            clearTimeout(tipTimer);
            return;
          }
          const target = e.target.closest('[data-tip]');
          if (target) void tooltip(target);
        });
      for (const event of ['mouseout', 'focusout'])
        root.addEventListener(event, (e) => {
          if (
            tooltipTarget &&
            (!(e.relatedTarget instanceof win.Node) ||
              (!tooltipTarget.contains(e.relatedTarget) && !tip.contains(e.relatedTarget)))
          )
            if (event === 'focusout') clearTip();
            else tipTimer = setTimeout(() => clearTip(), 180);
        });
      root.addEventListener('keydown', (event) => {
        const trigger = event.target.closest?.('[data-tip]');
        if (event.key === 'Escape' && tooltipTarget) {
          event.preventDefault();
          clearTip(tip.contains(root.activeElement));
        } else if (
          (event.key === 'F2' && (trigger || tooltipTarget)) ||
          (event.key === 'Enter' && trigger)
        ) {
          event.preventDefault();
          void tooltip(trigger || tooltipTarget);
          tip.focus({ preventScroll: true });
        }
      });
      root.addEventListener(
        'scroll',
        (event) => {
          if (!tip.contains(event.target)) clearTip();
        },
        true,
      );
      root.addEventListener(
        'error',
        (event) => {
          if (event.target instanceof win.HTMLImageElement) {
            event.target.classList.add('missing-art');
            event.target.closest('.meta-art,.meta-portrait')?.classList.add('art-failed');
          }
        },
        true,
      );
      win.addEventListener('resize', () => clearTip());
      doc.body.append(host);
    }
    async function completeAugments() {
      if (
        !model?.augmentEndpoint ||
        updating ||
        (model.rateCheckedAt && Date.now() - model.rateCheckedAt < MAX_AGE)
      )
        return;
      const snapshot = model,
        signal = contextAbort.signal;
      updating = true;
      const endpoint = new URL(snapshot.augmentEndpoint);
      endpoint.searchParams.set('table', 'ArenaWidget_table');
      let next = endpoint.href;
      const seen = new Set();
      let count = 0;
      try {
        while (
          next &&
          count++ < 25 &&
          !abort.signal.aborted &&
          model?.url === snapshot.url &&
          model?.patch === snapshot.patch
        ) {
          if (seen.has(next)) break;
          seen.add(next);
          const data = await fetchDocument(next, signal),
            rows = tableAugments(data, snapshot.url);
          if (!rows.length) throw Error('MetaSRC did not supply augment rates');
          if (model?.url !== snapshot.url || model?.patch !== snapshot.patch) break;
          const rates = new Map(rows.map((row) => [row.key, row]));
          model.augments = model.augments.map((row) => ({ ...row, ...rates.get(row.key) }));
          model.prismatics = model.prismatics.map((row) => ({ ...row, ...rates.get(row.key) }));
          const loader = data.querySelector('#ArenaWidget_table_loader[hx-get]');
          next = loader ? augmentURL(loader.getAttribute('hx-get'), snapshot.url) : '';
          renderAugments(true);
          renderPrismatics(true);
          saveCache(storage, model);
        }
        if (model?.url === snapshot.url) {
          if (!next) {
            model.rateCheckedAt = Date.now();
            saveCache(storage, model);
          }
          const missing = [...model.augments, ...model.prismatics].filter(
            (a) => a.pickRate === null,
          ).length;
          notice(missing ? 'Some pick rates are not reported; source tiers remain available.' : '');
        }
      } catch {
        // A context change aborts this pass on purpose and the finally block starts a fresh one;
        // only a real failure of the current pass is worth a notice.
        if (model?.url === snapshot.url && !signal.aborted)
          notice(
            'Some pick rates could not be refreshed. Source tiers and available rates are shown.',
          );
      } finally {
        updating = false;
        if (
          !abort.signal.aborted &&
          model &&
          (signal.aborted || model.url !== snapshot.url || model.patch !== snapshot.patch)
        )
          void completeAugments();
      }
    }
    function update() {
      if (abort.signal.aborted) return;
      let fresh;
      try {
        fresh = read(doc, currentHref());
      } catch {
        return;
      }
      let completed;
      if (pending) {
        if (pending.kind === 'build' && fresh.url !== pending.key) return;
        if (pending.kind === 'rune' && (!pending.changed || fresh.runes?.selected !== pending.key))
          return;
        completed = pending;
        endChange();
      }
      const cached = readCache(storage, fresh.url);
      // Keep already loaded page rates while a rune widget changes in the source.
      const previous =
        model?.url === fresh.url && model.patch === fresh.patch
          ? model
          : cached?.patch === fresh.patch
            ? cached
            : null;
      if (previous) {
        const rates = new Map(
          [...previous.augments, ...previous.prismatics].map((a) => [a.key, a]),
        );
        for (const kind of ['augments', 'prismatics'])
          fresh[kind] = fresh[kind].map((a) => ({
            ...a,
            pickRate: a.pickRate ?? rates.get(a.key)?.pickRate ?? null,
            games: a.games || rates.get(a.key)?.games || '',
          }));
        fresh.rateCheckedAt = previous.rateCheckedAt || 0;
      }
      const signature = JSON.stringify(fresh);
      if (model && JSON.stringify(model) === signature) return;
      if (!model || model.url !== fresh.url || model.patch !== fresh.patch) {
        contextAbort.abort();
        contextAbort = new AbortController();
        tipRequests.clear();
      }
      model = fresh;
      loaded.set(model.url, model);
      if (!host) mount();
      render();
      markChallengePath();
      if (completed)
        [...root.querySelectorAll('[data-build-style],[data-rune-page]')]
          .find(
            (button) =>
              button.dataset.buildStyle === completed.key ||
              button.dataset.runePage === completed.key,
          )
          ?.focus();
      saveCache(storage, model);
      notice('');
      if (model.mode === 'arena' && !updating) void completeAugments();
    }
    function start() {
      update();
      const sourceRoot = doc.body;
      if (!sourceRoot) return;
      observer = new win.MutationObserver((mutations) => {
        // Observe replacement of #main as well as its widgets, but do not reparse
        // hundreds of recommendations for unrelated ad/tooltip mutations.
        mutations = mutations.filter(
          (m) =>
            (m.target.nodeType === 1 && m.target.closest('#main')) ||
            (m.type === 'childList' &&
              [...m.addedNodes, ...m.removedNodes].some(
                (n) => n.nodeType === 1 && (n.id === 'main' || n.querySelector('#main')),
              )),
        );
        if (!mutations.length) return;
        if (
          pending?.kind === 'rune' &&
          pending.content &&
          mutations.some(
            (m) =>
              m.type === 'childList' &&
              (m.target === pending.content ||
                pending.content.contains(m.target) ||
                [...m.removedNodes].some(
                  (n) => n === pending.content || n.contains(pending.content),
                )),
          )
        )
          pending.changed = true;
        clearTimeout(updateTimer);
        // A requested build arrives as one swap: show it at once. Otherwise coalesce bursts.
        updateTimer = setTimeout(update, pending ? 16 : 150);
      });
      observer.observe(sourceRoot, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['data-tab-current-value'],
      });
      win.addEventListener('popstate', update);
      let challengeShown = false;
      challengeObserver = new win.MutationObserver(() => {
        markChallengePath();
        if (challenged() === challengeShown) return;
        challengeShown = challenged();
        if (challengeShown)
          notice('MetaSRC is asking for a quick human check. Complete it to load the build.', true);
        else if (pending) notice(`Loading ${pending.label}…`, true);
        else notice('');
      });
      challengeObserver.observe(doc.documentElement, { attributes: true, attributeFilter: ['class'] });
      for (const type of ['htmx:responseError', 'htmx:sendError', 'htmx:timeout'])
        doc.addEventListener(type, (event) => {
          if (pending && (event.target === pending.node || event.detail?.elt === pending.node))
            changeFailed(
              'The source could not load that selection. Previous recommendations are still shown.',
            );
        });
    }
    win.addEventListener(
      'beforeunload',
      () => {
        abort.abort();
        contextAbort.abort();
        if (pending) clearTimeout(pending.timer);
        clearTimeout(tipTimer);
        observer?.disconnect();
        challengeObserver?.disconnect();
        clearTimeout(updateTimer);
      },
      { once: true },
    );
    // The guide starts as soon as MetaSRC's #main has been parsed, well before the rest of the
    // page (DOMContentLoaded), and covers the page with its loading view from the first paint.
    let begun = false,
      early;
    const coverWhenUnreadable = () => {
      if (!model && !doc.querySelector('#shell-header')) uncover();
    };
    function begin() {
      if (begun) return;
      begun = true;
      early?.disconnect();
      start();
      if (doc.readyState === 'loading')
        doc.addEventListener(
          'DOMContentLoaded',
          () => {
            update();
            coverWhenUnreadable();
          },
          { once: true },
        );
      else coverWhenUnreadable();
    }
    function mainParsed() {
      // The parser only adds a sibling after #main (or after one of its ancestors) once #main is closed.
      for (let node = doc.getElementById('main'); node && node !== doc.body; node = node.parentElement)
        if (node.nextSibling) return true;
      return false;
    }
    function earlyStart() {
      if (!host && doc.body && doc.head) {
        try {
          mount();
          render();
        } catch {
          uncover();
        }
      }
      if (mainParsed()) begin();
    }
    applyAppearance();
    win.addEventListener(
      'load',
      () =>
        setTimeout(() => {
          if (!model) uncover();
        }, 8000),
      { once: true },
    );
    if (doc.readyState === 'loading') {
      early = new win.MutationObserver(earlyStart);
      early.observe(doc, { childList: true, subtree: true });
      earlyStart();
      doc.addEventListener('DOMContentLoaded', begin, { once: true });
    } else begin();
    return { refresh: update };
  }
  return {
    sourceURL,
    assetURL,
    apiURL,
    augmentURL,
    read,
    tableAugments,
    readStyles,
    sorted,
    percent,
    format,
    readCache,
    saveCache,
    clearCache,
    install,
  };
};
// The factory reads source metadata and repairs its cache without a network or DOM; the
// regression tests call these pure helpers directly.
globalThis.metasrcGuide = metasrcGuide();
