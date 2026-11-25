/** Small, bounded snapshots for an instant guide when F1 recreates the window. */
import { parseMayhemBuild, tencentPayload } from './mayhem-data.mjs';
const KEY = 'fryiloco-mayhem-guides-v1';
const MAX_AGE = 7 * 24 * 60 * 60;
function entries(storage, at) {
  try {
    const value = JSON.parse(storage.getItem(KEY) || '{}');
    if (value.schema !== 1 || !Array.isArray(value.entries)) return [];
    return value.entries
      .filter((response) => {
        const age = at / 1000 - response.build?.fetchedAt;
        if (!Number.isFinite(age) || age < 0 || age > MAX_AGE) return false;
        try {
          parseMayhemBuild(response.build.value, response.championId);
          return true;
        } catch {
          return false;
        }
      })
      .slice(0, 8);
  } catch {
    return [];
  }
}
export function readMayhemGuide(storage, championId, at = Date.now()) {
  return entries(storage, at).find((response) => response.championId === championId) ?? null;
}
export function saveMayhemGuide(storage, response, at = Date.now()) {
  try {
    parseMayhemBuild(response.build.value, response.championId);
    let rankings = null;
    if (response.rankings) {
      const data = tencentPayload(response.rankings.value);
      rankings = {
        ...response.rankings,
        value: {
          listcollect: String(data.listcollect ?? '')
            .split('#')
            .map((row) => row.split('_').slice(0, 5).join('_'))
            .join('#'),
          ...(data.dtstatdate ? { dtstatdate: String(data.dtstatdate) } : {}),
        },
      };
    }
    const compact = {
      championId: response.championId,
      build: response.build,
      rankings,
      rankingDate: response.rankingDate,
      notices: [],
    };
    const remaining = entries(storage, at).filter((row) => row.championId !== response.championId);
    storage.setItem(
      KEY,
      JSON.stringify({ schema: 1, entries: [compact, ...remaining].slice(0, 8) }),
    );
  } catch {
    /* Storage being unavailable never prevents the live guide. */
  }
}
