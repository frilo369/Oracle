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
}
impl Catalogue {
    pub fn bundled() -> Self {
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
                    row["apiName"]
                        .as_str()
                        .is_some_and(|n| n.starts_with("ARAM_")),
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
        let names = names
            .into_iter()
            .filter_map(|(name, (_, id))| (id != 0).then_some((name, id)))
            .collect();
        Self { names }
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
