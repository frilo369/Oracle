import { invoke } from "./mayhem-ipc.mjs";
import {
  formatPercent,
  normaliseCatalogue,
  parseMayhemBuild,
  parseMayhemRankings,
  plainText,
  itemDescription,
  resolveGameEntry,
  sortEquipment,
  groupMayhemAugments,
  artSources
} from "./mayhem-data.mjs";
import { readMayhemGuide, saveMayhemGuide } from "./mayhem-cache.mjs";
function element(selector) {
  const node = document.querySelector(selector);
  if (!node) throw new Error(`Missing guide element: ${selector}`);
  return node;
}
const select = element("#champion-select"), refresh = element("#refresh");
const search = element("#augment-search"), skillSelect = element("#skill-select");
const equipmentSort = {
  core: "winRate",
  situational: "pickRate",
  boots: "pickRate"
};
const content = element("#guide-content"), loading = element("#loading"), errorCard = element("#error"), notice = element("#notice"), tooltip = element("#item-tooltip");
const storage = {
  getItem: (key) => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value)
};
let catalogue = { champions: {}, items: {}, augments: {} }, fallback = catalogue;
let build = null, lastResponse = null, checkingSaved = false;
let currentChampion = Number(
  new URL(location.href).searchParams.get("champion")
), revision = 0, tooltipTarget = null;
let tooltipTimer = 0, restoringTooltipFocus = false;
const cachedResponses = /* @__PURE__ */ new Map();
const escape = (value) => String(value ?? "").replace(
  /[&<>"']/g,
  (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]
);
const entry = (kind, id) => resolveGameEntry(catalogue, kind, id);
const initials = (name) => name.split(/\s+/).slice(0, 2).map((word) => word[0]).join("");
const guideArt = (kind, item) => artSources(catalogue, kind, item).map((url) =>
  url.startsWith("./art/") ? `https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/${url.slice(6)}` : url
);
// A picture tries its addresses in order (artSources); the error handler below moves on to the next.
const art = (item, kind) => {
  const [first, ...rest] = guideArt(kind, item);
  return `${first ? `<img src="${escape(first)}" data-next="${escape(rest.join(" "))}" alt="" loading="lazy" decoding="async">` : ""}<span class="art-fallback${first ? "" : " art-absent"}" aria-hidden="true">${escape(initials(item.name))}</span>`;
};
function nextArt(image) {
  const [next, ...rest] = (image.dataset.next || "").split(" ").filter(Boolean);
  if (!next) return false;
  image.dataset.next = rest.join(" ");
  image.src = next;
  return true;
}
const describe = (item) => [item.name, itemDescription(item.description)].filter(Boolean).join("\n\n");
function rate(win, pick, showBar = false) {
  const bar = Math.max(0, Math.min(100, win ?? 0));
  const track = showBar ? `<span class="win-track" aria-hidden="true"><span style="width:${bar}%"></span></span>` : "";
  return `<div class="build-rate"><strong>${formatPercent(win)} <small>WR</small></strong>${track}<span class="pick-rate">${formatPercent(pick)}<small class="pick-label"> pick</small></span></div>`;
}
function setNotice(messages, checking = false) {
  notice.textContent = messages.join(" ");
  notice.hidden = !messages.length;
  notice.classList.toggle("updating", checking);
}
function setBusy(busy) {
  refresh.disabled = busy;
  select.disabled = busy;
  loading.hidden = !busy || !content.hidden;
  document.body.classList.toggle("refreshing", busy);
  content.setAttribute("aria-busy", String(busy));
}
function championHeader(id) {
  const champion = entry("champions", id);
  element("#champion-name").textContent = champion.name;
  document.title = `${champion.name} \xB7 ARAM: Mayhem \xB7 Oracle`;
  const icon = element("#champion-icon");
  icon.classList.remove("missing-art");
  icon.parentElement.classList.toggle("art-failed", !champion.icon);
  element("#champion-art-fallback").textContent = initials(champion.name);
  icon.hidden = !champion.icon;
  const [first, ...rest] = guideArt("champions", champion);
  icon.dataset.next = rest.join(" ");
  if (first) icon.src = first;
  select.value = String(id);
}
function fillChampions() {
  select.innerHTML = Object.values(catalogue.champions).sort((a, b) => a.name.localeCompare(b.name, "en")).map((row) => `<option value="${row.id}">${escape(row.name)}</option>`).join("");
  select.disabled = document.body.classList.contains("refreshing");
  select.value = String(currentChampion);
}
function renderAugments() {
  if (!build) return;
  const groups = groupMayhemAugments(build.augments, catalogue, search.value);
  element("#augment-count").textContent = String(
    groups.reduce((sum, group) => sum + group.rows.length, 0)
  );
  for (const group of groups) {
    const key = group.rarity.toLowerCase();
    element(`#${key}-count`).textContent = String(group.rows.length);
    const list = element(`#${key}-rows`), scroll = list.scrollTop;
    list.innerHTML = group.rows.map((row) => {
      const game = entry("augments", row.id);
      return `<article class="augment-row" aria-label="${escape(game.name)}"><span class="augment-art">${art(game, "augments")}</span><span class="entry-name">${escape(game.name)}</span><div class="augment-metrics"><span class="grade grade-${escape(row.grade)}" title="Tencent grade ${escape(row.grade || "unavailable")}">${escape(row.grade || "\u2014")}</span><span class="augment-pick" title="Pick rate">${formatPercent(row.pickRate)}</span></div></article>`;
    }).join("") || '<p class="empty">No matching recommendations.</p>';
    list.scrollTop = scroll;
  }
}
function itemRow(row, sectionId, rank) {
  if (sectionId === "boots" && row.itemIds.length === 1) {
    const item = entry("items", row.itemIds[0]);
    return `<div class="build-row boot-choice"><span class="item" data-item="${item.id}" tabindex="0" aria-label="${escape(item.name)}">${art(item, "items")}<span class="boot-copy"><span class="item-name">${escape(item.name)}</span>${rate(row.winRate, row.pickRate)}</span></span></div>`;
  }
  const isCore = sectionId === "core";
  const medal = rank < 3 ? ` rank-medal rank-medal-${rank + 1}` : "";
  const items = row.itemIds.map((id, index) => {
    const item = entry("items", id);
    return `${isCore && index ? '<span class="build-arrow" aria-hidden="true">\u203A</span>' : ""}<span class="item" data-item="${id}" tabindex="0" aria-label="${escape(item.name)}">${art(item, "items")}<span class="item-name">${escape(item.name)}</span></span>`;
  }).join("");
  const badge = isCore ? `<span class="build-rank${medal}" aria-hidden="true">${rank + 1}</span>` : "";
  return `<div class="build-row${isCore ? " core-build" : ""}">${badge}<div class="item-group">${items}</div>${rate(row.winRate, row.pickRate, isCore)}</div>`;
}
function renderEquipment(resetSection) {
  if (!build) return;
  const positions = new Map(
    Array.from(document.querySelectorAll(".equipment-list")).map(
      (node) => [node.dataset.section, node.scrollTop]
    )
  );
  element("#equipment").innerHTML = build.sections.map((section) => {
    const selectedSort = equipmentSort[section.id] || "winRate";
    const rows = sortEquipment(section.rows, selectedSort);
    const controls = section.id === "boots" ? `<span class="count">${rows.length} options</span><label class="build-sort boots-sort">Sort<select data-boots-sort aria-label="Sort Boots"><option value="winRate"${selectedSort === "winRate" ? " selected" : ""}>Win rate</option><option value="pickRate"${selectedSort === "pickRate" ? " selected" : ""}>Pick rate</option></select></label>` : `<span class="section-sort" role="group" aria-label="Sort ${section.title}">${[
      ["winRate", "Win Rate"],
      ["pickRate", "Pick Rate"]
    ].map(
      ([value, label]) => `<button type="button" data-sort-section="${section.id}" data-sort-value="${value}" aria-pressed="${selectedSort === value}">${label}</button>`
    ).join("")}</span>`;
    const content2 = rows.length ? rows.map((row, index) => itemRow(row, section.id, index)).join("") : '<p class="empty">No recommendations supplied.</p>';
    const mark = section.id === "core" ? "core" : section.id === "boots" ? "boots" : "other";
    return `<section class="equipment-section section-${section.id}">
        <h3><svg class="section-icon" aria-hidden="true"><use href="#mark-${mark}"/></svg><span class="equipment-title">${section.title}</span>${controls}</h3>
        <div class="equipment-list" data-section="${section.id}" tabindex="0" role="region" aria-label="${section.title}">${content2}</div>
      </section>`;
  }).join("");
  for (const node of Array.from(
    document.querySelectorAll(".equipment-list")
  ))
    node.scrollTop = node.dataset.section === resetSection ? 0 : positions.get(node.dataset.section) || 0;
}
function renderSkills() {
  if (!build) return;
  const skill = build.skillOrders[Number(skillSelect.value) || 0];
  if (!skill) {
    element("#skills").innerHTML = '<p class="empty">No skill order supplied.</p>';
    return;
  }
  const plan = skill.plans[0];
  element("#skills").innerHTML = `<div class="skill-summary"><div class="skill-rates">${formatPercent(skill.winRate)} win<span>${formatPercent(skill.pickRate)} pick</span></div></div>${plan ? `<div class="skill-upgrades" aria-label="First ${plan.order.length} skill upgrades reported by Tencent">${plan.order.map((key, index) => `<div class="skill-upgrade"><small>${index + 1}</small><span class="skill-key skill-${key}">${key}</span></div>`).join("")}</div>` : ""}`;
}
function renderResponse(response, checking = false) {
  if (response.championId !== currentChampion) return;
  build = parseMayhemBuild(response.build.value, currentChampion);
  lastResponse = response;
  checkingSaved = checking;
  const messages = Array.isArray(response.notices)
    ? response.notices.filter(message => typeof message === "string") : [];
  championHeader(currentChampion);
  const stale = response.build.stale === true;
  if (checking)
    messages.unshift("Saved recommendations \xB7 checking for fresh data\u2026");
  else if (stale)
    messages.unshift(
      "Showing cached recommendations. Tencent is temporarily unavailable."
    );
  if (Date.now() - Date.parse(`${build.dataDate}T00:00:00+08:00`) > 3 * 24 * 60 * 60 * 1e3)
    messages.push("This dataset is more than three days old.");
  element("#data-date").textContent = `${checking ? "Saved \xB7 " : stale ? "Cached \xB7 " : ""}${build.dataDate}`;
  let stats = "";
  if (response.rankings)
    try {
      const ranks = parseMayhemRankings(
        response.rankings.value,
        response.rankingDate
      );
      if (ranks.dataDate !== build.dataDate)
        throw new Error("Different data dates");
      const champion = ranks.champions.find(
        (row) => row.id === currentChampion
      );
      if (champion)
        stats = `<div class="stat"><strong>${formatPercent(champion.winRate)}</strong><span>Win rate</span></div><div class="stat"><strong>${formatPercent(champion.pickRate)}</strong><span>Pick rate</span></div><div class="stat"><strong>#${champion.rank}</strong><span>Rank</span></div>`;
    } catch {
      messages.push("Champion rankings are unavailable for this data date.");
    }
  element("#champion-stats").innerHTML = stats ? `<svg class="section-icon stats-mark" aria-hidden="true"><use href="#mark-stats"/></svg>${stats}` : '<span class="count">Rankings unavailable</span>';
  const missing = /* @__PURE__ */ new Set();
  for (const row of build.augments)
    if (!catalogue.augments[row.id]) missing.add("augment");
  for (const section of build.sections)
    for (const row of section.rows)
      for (const id of row.itemIds)
        if (!catalogue.items[id]) missing.add("item");
  if (missing.size)
    messages.push(
      `Some new ${[...missing].join(" and ")} names are not available in English game data yet; their IDs are shown explicitly.`
    );
  setNotice([...new Set(messages)], checking);
  const previousSkill = skillSelect.value;
  skillSelect.innerHTML = build.skillOrders.map(
    (row, index) => `<option value="${index}">${row.priority.join(" \u2192 ")}</option>`
  ).join("");
  skillSelect.disabled = !build.skillOrders.length;
  if (Number(previousSkill) < build.skillOrders.length)
    skillSelect.value = previousSkill || "0";
  renderAugments();
  renderEquipment();
  renderSkills();
  element("#skills-strip").hidden = false;
  errorCard.hidden = true;
  content.hidden = false;
}
async function loadChampion(id, forceRefresh = false) {
  // Any real champion number, as the backend checks it: a champion newer than the bundled
  // list still loads, and gets its English name when League's own list arrives.
  if (!Number.isSafeInteger(id) || id <= 0 || id > 10000) {
    loading.hidden = true;
    errorCard.hidden = false;
    element("#error-message").textContent = "Choose a champion to load their Mayhem guide.";
    return;
  }
  const request = ++revision;
  currentChampion = id;
  hideTooltip();
  history.replaceState(null, "", `?champion=${id}`);
  championHeader(id);
  errorCard.hidden = true;
  content.hidden = true;
  element("#skills-strip").hidden = true;
  setNotice([]);
  element("#champion-stats").textContent = "";
  element("#data-date").textContent = "Loading recommendations\u2026";
  const saved = cachedResponses.get(id) || readMayhemGuide(storage, id);
  if (saved) renderResponse(saved, true);
  setBusy(true);
  try {
    const result = await invoke("mayhem_guide", {
      championId: id,
      forceRefresh
    });
    if (request !== revision) return;
    if (!result.ok)
      throw new Error(
        result.error?.detail || "Could not load Tencent\u2019s Mayhem data."
      );
    if (result.value.championId !== id)
      throw new Error("Received a guide for a different champion.");
    renderResponse(result.value);
    cachedResponses.set(id, result.value);
    saveMayhemGuide(storage, result.value);
  } catch (error) {
    if (request !== revision) return;
    const previous = cachedResponses.get(id) || saved;
    if (previous)
      renderResponse({
        ...previous,
        build: { ...previous.build, stale: true }
      });
    else {
      errorCard.hidden = false;
      element("#error-message").textContent = "umbraGuide" in window ? plainText(error instanceof Error ? error.message : String(error)) : "Open this guide with F1 in a Mayhem match to load live recommendations.";
      element("#data-date").textContent = "No data loaded";
    }
  } finally {
    if (request === revision) setBusy(false);
  }
}
async function refreshCatalogue() {
  try {
    const result = await invoke(
      "mayhem_catalogue"
    );
    if (!result.ok) return;
    const latest = normaliseCatalogue(
      result.value.sources
    );
    const merged = {
      champions: { ...fallback.champions, ...latest.champions },
      items: { ...fallback.items, ...latest.items },
      augments: { ...fallback.augments, ...latest.augments }
    };
    if (JSON.stringify(merged) === JSON.stringify(catalogue)) return;
    catalogue = merged;
    fillChampions();
    if (lastResponse?.championId === currentChampion)
      renderResponse(lastResponse, checkingSaved);
    else if (Number.isSafeInteger(currentChampion) && currentChampion > 0)
      championHeader(currentChampion);
  } catch {
  }
}
function hideTooltip(restoreFocus = false) {
  window.clearTimeout(tooltipTimer);
  const target = tooltipTarget;
  tooltip.hidden = true;
  tooltipTarget?.removeAttribute("aria-describedby");
  tooltipTarget = null;
  if (restoreFocus && target?.isConnected) {
    restoringTooltipFocus = true;
    target.focus({ preventScroll: true });
    restoringTooltipFocus = false;
  }
}
function showTooltip(target) {
  window.clearTimeout(tooltipTimer);
  if (restoringTooltipFocus) return;
  if (tooltipTarget === target) return;
  hideTooltip();
  tooltipTarget = target;
  tooltip.textContent = describe(entry("items", Number(target.dataset.item)));
  tooltip.hidden = false;
  target.setAttribute("aria-describedby", "item-tooltip");
  const box = target.getBoundingClientRect(), width = tooltip.offsetWidth, height = tooltip.offsetHeight;
  tooltip.style.left = `${Math.max(12, Math.min(document.documentElement.clientWidth - width - 12, box.left))}px`;
  tooltip.style.top = `${box.bottom + height + 12 < innerHeight ? box.bottom + 9 : Math.max(12, box.top - height - 9)}px`;
}
for (const name of ["mouseover", "focusin"])
  document.addEventListener(name, (event) => {
    if (tooltip.contains(event.target)) {
      window.clearTimeout(tooltipTimer);
      return;
    }
    const target = event.target.closest(
      "[data-item]"
    );
    if (target) showTooltip(target);
  });
for (const name of ["mouseout", "focusout"])
  document.addEventListener(name, (event) => {
    const related = event.relatedTarget;
    if (tooltipTarget && (!(related instanceof Node) || !tooltipTarget.contains(related) && !tooltip.contains(related)))
      if (name === "focusout") hideTooltip();
      else tooltipTimer = window.setTimeout(() => hideTooltip(), 180);
  });
document.addEventListener("keydown", (event) => {
  const trigger = event.target instanceof HTMLElement ? event.target.closest("[data-item]") : null;
  if (event.key === "Escape" && tooltipTarget) {
    event.preventDefault();
    hideTooltip(tooltip.contains(document.activeElement));
  } else if (event.key === "F2" && (trigger || tooltipTarget) || event.key === "Enter" && trigger) {
    event.preventDefault();
    showTooltip(trigger || tooltipTarget);
    tooltip.focus({ preventScroll: true });
  }
});
document.addEventListener(
  "scroll",
  (event) => {
    if (!tooltip.contains(event.target)) hideTooltip();
  },
  true
);
window.addEventListener("resize", () => hideTooltip());
select.addEventListener("change", () => {
  void loadChampion(Number(select.value));
});
refresh.addEventListener(
  "click",
  () => void loadChampion(currentChampion, true)
);
element("#retry").addEventListener(
  "click",
  () => void loadChampion(currentChampion, true)
);
search.addEventListener("input", renderAugments);
element("#equipment").addEventListener("click", (event) => {
  const button = event.target.closest(
    "button[data-sort-section]"
  );
  if (!button) return;
  const section = button.dataset.sortSection || "";
  const value = button.dataset.sortValue;
  if (!["core", "situational"].includes(section) || value !== "winRate" && value !== "pickRate")
    return;
  equipmentSort[section] = value;
  hideTooltip();
  renderEquipment(section);
  element(
    `[data-sort-section="${section}"][data-sort-value="${value}"]`
  ).focus();
});
element("#equipment").addEventListener("change", (event) => {
  if (!(event.target instanceof HTMLSelectElement) || !event.target.matches("[data-boots-sort]"))
    return;
  const value = event.target.value;
  if (value !== "winRate" && value !== "pickRate") return;
  equipmentSort.boots = value;
  hideTooltip();
  renderEquipment("boots");
  element("[data-boots-sort]").focus();
});
skillSelect.addEventListener("change", renderSkills);
document.addEventListener(
  "error",
  (event) => {
    if (event.target instanceof HTMLImageElement) {
      if (nextArt(event.target)) return;
      event.target.classList.add("missing-art");
      event.target.closest(".item,.augment-art,[data-art]")?.classList.add("art-failed");
    }
  },
  true
);
// Oracle shows the next match's champion here without reloading the page and its catalogue.
let catalogueState = "loading";
window.umbraShowChampion = (id) => {
  id = Number(id);
  if (catalogueState === "failed") location.replace(`?champion=${id}`);
  else if (catalogueState === "loading") currentChampion = id;
  else if (id !== currentChampion || !build) void loadChampion(id);
};
void (async () => {
  try {
    const response = await fetch("./mayhem-catalogue.json");
    if (!response.ok)
      throw new Error("The bundled English catalogue could not be loaded.");
    fallback = await response.json();
    if (!fallback.champions || !fallback.items || !fallback.augments)
      throw new Error("The English catalogue is incomplete.");
    catalogue = fallback;
    fillChampions();
    catalogueState = "ready";
    void loadChampion(currentChampion);
    void refreshCatalogue();
  } catch (error) {
    catalogueState = "failed";
    setBusy(false);
    errorCard.hidden = false;
    element("#error-message").textContent = plainText(
      error instanceof Error ? error.message : String(error)
    );
  }
})();
