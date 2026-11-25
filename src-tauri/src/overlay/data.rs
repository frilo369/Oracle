use serde_json::Value;
use std::collections::HashMap;

pub fn key(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

pub struct Catalogue {
    pub names: HashMap<String, u64>,
    ocr_names: HashMap<String, u64>,
}
fn ocr_key(s: &str) -> String {
    key(s)
        .replace("rn", "m")
        .replace("vv", "w")
        .replace("iv", "w")
        .replace("tw", "w")
}
fn one_away(a: &[u8], b: &[u8]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else {
            edits += 1;
            if edits > 1 {
                return false;
            }
            if a.len() >= b.len() {
                i += 1;
            }
            if b.len() >= a.len() {
                j += 1;
            }
        }
    }
    edits + (i < a.len() || j < b.len()) as usize <= 1
}
impl Catalogue {
    pub fn resolve(&self, text: &str, ocr: bool) -> Option<u64> {
        let exact = key(text);
        if let Some(id) = self.names.get(&exact) {
            return Some(*id);
        }
        if !ocr {
            return None;
        }
        let k = ocr_key(text);
        if let Some(id) = self.ocr_names.get(&k) {
            return Some(*id);
        }
        if !(5..=36).contains(&k.len()) {
            return None;
        }
        let mut found = None;
        for (name, id) in &self.ocr_names {
            if one_away(k.as_bytes(), name.as_bytes()) {
                if found.is_some_and(|old| old != *id) {
                    return None;
                }
                found = Some(*id);
            }
        }
        found
    }
    pub fn bundled() -> Self {
        Self::for_mode(false)
    }
    pub fn for_mode(arena: bool) -> Self {
        let doc: Value =
            serde_json::from_str(include_str!("../../../src/ui/guide/mayhem-catalogue.json"))
                .expect("bundled catalogue");
        let mut names = HashMap::new();
        for (id, row) in doc["augments"].as_object().unwrap() {
            let Ok(id) = id.parse() else { continue };
            for name in [
                row["name"].as_str().unwrap_or(""),
                row["apiName"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches("ARAM_"),
            ] {
                let k = key(name);
                if k.len() < 4 {
                    continue;
                }
                let priority = u8::from(
                    arena
                        != (row["apiName"]
                            .as_str()
                            .is_some_and(|n| n.starts_with("ARAM_"))),
                );
                names
                    .entry(k)
                    .and_modify(|old: &mut (u8, u64)| {
                        if priority > old.0 {
                            *old = (priority, id);
                        } else if priority == old.0 && old.1 != id {
                            old.1 = 0;
                        }
                    })
                    .or_insert((priority, id));
            }
        }
        let names: HashMap<String, u64> = names
            .into_iter()
            .filter_map(|(name, (_, id))| (id != 0).then_some((name, id)))
            .collect();
        let mut ocr_names = HashMap::new();
        for (name, id) in &names {
            ocr_names
                .entry(ocr_key(name))
                .and_modify(|old| {
                    if *old != *id {
                        *old = 0
                    }
                })
                .or_insert(*id);
        }
        ocr_names.retain(|_, id| *id != 0);
        Self { names, ocr_names }
    }
}

#[derive(Clone, Debug)]
pub struct Stat {
    pub pick: Option<f64>,
    pub grade: String,
}
#[derive(Default)]
pub struct Stats {
    pub rows: HashMap<u64, Stat>,
    pub stale: bool,
}
impl Stats {
    pub fn arena(response: &Value, champion: u64, catalogue: &Catalogue) -> Result<Self, String> {
        if response["championId"].as_u64() != Some(champion) || response["mode"] != "arena" {
            return Err("Arena statistics have a different identity".into());
        }
        let mut rows = HashMap::new();
        for row in response["rows"]
            .as_array()
            .ok_or("Missing Arena statistics")?
            .iter()
            .take(1000)
        {
            let Some(id) = row["name"]
                .as_str()
                .and_then(|name| catalogue.names.get(&key(name)))
            else {
                continue;
            };
            let pick = row["pick"]
                .as_str()
                .and_then(|s| s.trim().strip_suffix('%'))
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
            let grade = row["grade"]
                .as_str()
                .filter(|s| {
                    matches!(
                        *s,
                        "S+" | "S" | "A+" | "A" | "B+" | "B" | "C+" | "C" | "D+" | "D" | "F"
                    )
                })
                .unwrap_or("")
                .to_string();
            if pick.is_some() || !grade.is_empty() {
                rows.insert(*id, Stat { pick, grade });
            }
        }
        if rows.is_empty() {
            return Err("Arena statistics are loading".into());
        }
        Ok(Self { rows, stale: false })
    }
    pub fn parse(response: &Value, champion: u64) -> Result<Self, String> {
        if response["championId"].as_u64() != Some(champion) {
            return Err("Different champion".into());
        }
        let doc = crate::guide::tencent_payload(&response["build"]["value"], "dtstatdate")
            .ok_or("Missing augment data")?;
        let source = doc["augment_json_irank"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| doc["augment_json"].as_str())
            .unwrap_or("");
        let mut rows = HashMap::new();
        for group in source.split('&') {
            let content = group.split_once(':').map_or(group, |(_, s)| s);
            for row in content.split('#') {
                let parts: Vec<_> = row.split('|').collect();
                if parts.len() < 5 {
                    continue;
                }
                if parts[1].parse::<u64>().ok() != Some(champion) {
                    return Err("Different champion in augment data".into());
                }
                let Ok(id) = parts[2].parse::<u64>() else {
                    continue;
                };
                let pick = parts[4]
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                    .map(|n| n * 100.0);
                let grade = parts
                    .get(5)
                    .copied()
                    .filter(|s| matches!(*s, "S" | "A" | "B" | "C" | "D"))
                    .unwrap_or("")
                    .to_string();
                rows.entry(id).or_insert(Stat { pick, grade });
            }
        }
        if rows.is_empty() {
            return Err("Missing augment statistics".into());
        }
        Ok(Self {
            rows,
            stale: response["build"]["stale"] == true,
        })
    }
    pub fn label(&self, id: u64) -> String {
        match self.rows.get(&id) {
            Some(s) => format!(
                "Pick {}   |   Tier {}{}",
                s.pick
                    .map(|v| format!("{v:.2}%"))
                    .unwrap_or_else(|| "—".into()),
                if s.grade.is_empty() { "—" } else { &s.grade },
                if self.stale { "  · Cached" } else { "" }
            ),
            None => "Pick —   |   Tier —".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stats_are_champion_specific_and_do_not_invent_win_rates() {
        let root = tempfile::tempdir().unwrap();
        let response = crate::guide::read(root.path(), 1, false, true).unwrap();
        let stats = Stats::parse(&response, 1).unwrap();
        assert_eq!(stats.label(3055), "Pick 13.85%   |   Tier S");
        assert!(Stats::parse(&response, 103).is_err());
        assert_eq!(stats.label(999999), "Pick —   |   Tier —");
    }
    #[test]
    fn catalogue_matches_punctuation_and_case() {
        let c = Catalogue::bundled();
        assert_eq!(c.names.get(&key("Ultimate Recursion")), Some(&3055));
        assert_eq!(c.names.get(&key("ADAPT")), Some(&1205));
        let arena = Catalogue::for_mode(true);
        assert_eq!(arena.names.get(&key("ADAPT")), Some(&205));
        assert_eq!(arena.names.get(&key("Recursion")), Some(&68));
    }
    #[test]
    fn arena_statistics_never_reuse_mayhem_percentages() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/overlay-arena.json"))
                .unwrap();
        let c = Catalogue::for_mode(true);
        let stats = Stats::arena(&fixture, 1, &c).unwrap();
        assert_eq!(stats.label(205), "Pick 11.11%   |   Tier S");
        assert_eq!(stats.label(1205), "Pick —   |   Tier —");
        assert!(Stats::arena(&fixture, 103, &c).is_err());
    }
    #[test]
    fn unavailable_percentages_and_stale_data_are_explicit() {
        let root = tempfile::tempdir().unwrap();
        let mut response = crate::guide::read(root.path(), 1, false, true).unwrap();
        response["build"]["stale"] = Value::Bool(true);
        let mut stats = Stats::parse(&response, 1).unwrap();
        assert!(stats.label(3055).ends_with("· Cached"));
        stats.rows.insert(
            1,
            Stat {
                pick: None,
                grade: String::new(),
            },
        );
        assert!(stats.label(1).starts_with("Pick —   |   Tier —"));
        response["build"]["value"] = serde_json::json!({"dtstatdate":"20261125","augment_json":""});
        assert!(Stats::parse(&response, 1).is_err());
    }
}
