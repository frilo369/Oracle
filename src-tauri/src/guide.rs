use crate::settings::save;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ENGLISH: &str =
    "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/";
const TENCENT_AUGMENTS: &str =
    "https://game.gtimg.cn/images/lol/act/img/js/kiwi/kiwi_augments.json";
const DAY: u64 = 86_400;
const MAX_SOURCE: u64 = 6 * 1024 * 1024;
const MAX_CACHE: u64 = 12 * 1024 * 1024;

pub fn capped_json(response: reqwest::blocking::Response, limit: u64) -> Result<Value, String> {
    let mut data = Vec::new();
    response
        .error_for_status()
        .map_err(|e| format!("HTTP request failed ({:?})", e.status()))?
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Could not read response")?;
    if data.len() as u64 > limit {
        return Err("Response exceeds size limit".into());
    }
    serde_json::from_slice(&data).map_err(|_| "Invalid response data".into())
}

fn catalogue_list(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|rows| rows.len() >= 100 && rows.iter().all(Value::is_object))
}

fn catalogue_source(
    path: &Path,
    url: &str,
    now: u64,
    local: Option<Value>,
    fetch: &impl Fn(&str) -> Result<Value, String>,
) -> Result<(Value, bool), String> {
    if let Some(local) = local.filter(catalogue_list) {
        return Ok((local, false));
    }
    let cached: Option<Value> = fs::File::open(path).ok().and_then(|file| {
        let mut bytes = Vec::new();
        file.take(MAX_CACHE + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_CACHE {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    });
    let cached = cached.and_then(|mut doc| {
        let age = now.checked_sub(doc["fetchedAt"].as_u64()?)?;
        (doc["url"] == url && age <= 7 * DAY && catalogue_list(&doc["value"]))
            .then(|| (doc["value"].take(), age))
    });
    if let Some((value, age)) = &cached {
        if *age < DAY {
            return Ok((value.clone(), false));
        }
    }
    let fresh = fetch(url).and_then(|value| {
        catalogue_list(&value)
            .then_some(value)
            .ok_or_else(|| "Incomplete Mayhem catalogue".into())
    });
    match fresh {
        Ok(value) => {
            let _ = save(path, &json!({"url":url,"fetchedAt":now,"value":&value}));
            Ok((value, false))
        }
        Err(error) => cached.map(|(value, _)| (value, true)).ok_or(error),
    }
}

pub fn catalogue(root: &Path, fixture: bool) -> Result<Value, String> {
    if fixture {
        return Ok(json!({"sources":{},"stale":false}));
    }
    let cache = root.join("mayhem-catalogue");
    fs::create_dir_all(&cache).map_err(|error| error.to_string())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let http = reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent("Oracle/MayhemGuide")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| error.to_string())?;
    let fetch = |url: &str| {
        let response = http.get(url).send().map_err(|error| error.to_string())?;
        capped_json(response, MAX_SOURCE)
    };
    let english = |name: &str| {
        let local = crate::guide_assets::game_file(&format!("v1/{name}"))
            .and_then(|(bytes, _)| serde_json::from_slice(&bytes).ok());
        catalogue_source(
            &cache.join(name),
            &format!("{ENGLISH}{name}"),
            now,
            local,
            &fetch,
        )
    };
    std::thread::scope(|scope| {
        let champions = scope.spawn(|| english("champion-summary.json"));
        let items = scope.spawn(|| english("items.json"));
        let augments = scope.spawn(|| english("cherry-augments.json"));
        let tencent = catalogue_source(
            &cache.join("tencent-augments.json"),
            TENCENT_AUGMENTS,
            now,
            None,
            &fetch,
        )
        .map(|(value, _)| value)
        .unwrap_or_else(|_| json!([]));
        let [champions, items, augments] = [champions, items, augments].map(|task| {
            task.join()
                .map_err(|_| "Mayhem catalogue reader stopped".to_string())
                .and_then(|result| result)
        });
        let ((champions, c), (items, i), (augments, a)) = (champions?, items?, augments?);
        Ok(
            json!({"sources":{"champions":champions,"items":items,"augments":augments,"tencentAugments":tencent},"stale":c || i || a}),
        )
    })
}
pub(crate) fn tencent_payload(payload: &Value, field: &str) -> Option<Value> {
    let mut v = payload.clone();
    for _ in 0..7 {
        if let Some(s) = v.as_str() {
            v = serde_json::from_str(s).ok()?;
        }
        if v.get("code")
            .is_some_and(|code| !code.is_null() && code != 0 && code != "0")
            || v["result"].as_i64().is_some_and(|code| code != 0)
        {
            return None;
        }
        if v.get(field).is_some() {
            return Some(v);
        }
        if let Some(fields) = v
            .pointer("/data/_fieldValues")
            .and_then(Value::as_object)
            .filter(|f| f.len() == 1)
        {
            v = fields.values().next()?.clone();
        } else if !v["data"].is_null() {
            v = v["data"].clone();
        } else if !v["result"].is_null() {
            v = v["result"].clone();
        } else {
            break;
        }
    }
    None
}

fn date(payload: &Value) -> Result<String, String> {
    let data =
        tencent_payload(payload, "dtstatdate").ok_or("Source returned no Mayhem build date")?;
    let has_build = [
        "augment_json_irank",
        "augment_json",
        "itemone_json",
        "itemcore_json",
        "itemshoes",
    ]
    .iter()
    .any(|field| data[*field].as_str().is_some_and(|value| !value.is_empty()));
    if !has_build {
        return Err("Tencent has no Mayhem build for this champion yet.".into());
    }
    data["dtstatdate"]
        .as_str()
        .filter(|date| valid_date(date))
        .map(str::to_owned)
        .ok_or_else(|| "Source returned no Mayhem build date".into())
}

fn valid_date(date: &str) -> bool {
    if date.len() != 8 || !date.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap();
    let month: usize = date[4..6].parse().unwrap();
    let day: u32 = date[6..].parse().unwrap();
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        0,
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    month < days.len() && day > 0 && day <= days[month]
}

fn positive_id(value: &str) -> Option<u64> {
    (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .then(|| value.parse::<u64>().ok())
        .flatten()
        .filter(|id| (1..=9_007_199_254_740_991).contains(id))
}

fn has_item_ids(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|id| (1..=9_007_199_254_740_991).contains(&id))
        || value
            .as_str()
            .is_some_and(|ids| ids.split([',', '&']).any(|id| positive_id(id).is_some()))
}

fn decoded_rows(value: &Value) -> Option<Value> {
    let mut value = value.clone();
    for _ in 0..5 {
        let Some(text) = value.as_str() else {
            break;
        };
        value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(_) => {
                let unescaped = text.replace("\\\"", "\"").replace("\\\\", "\\");
                if unescaped == text {
                    return None;
                }
                Value::String(unescaped)
            }
        };
    }
    value.is_object().then_some(value)
}

/// Do not replace a good offline cache with a response the guide cannot render.
fn build_date(payload: &Value, champion: u64) -> Result<String, String> {
    let day = date(payload)?;
    let data = tencent_payload(payload, "dtstatdate").ok_or("Unreadable Mayhem build")?;
    let mut usable = false;
    let augments = data["augment_json_irank"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| data["augment_json"].as_str())
        .unwrap_or("");
    for group in augments.split('&') {
        let rows = group.split_once(':').map_or(group, |(_, rows)| rows);
        for row in rows.split('#') {
            let fields: Vec<_> = row.split('|').collect();
            if fields.len() < 3
                || positive_id(fields[0]).is_none()
                || positive_id(fields[2]).is_none()
            {
                continue;
            }
            if positive_id(fields[1]) != Some(champion) {
                return Err("Tencent returned a build for a different champion.".into());
            }
            usable = true;
        }
    }
    for (field, items) in [("itemone_json", "itemone"), ("itemcore_json", "itemcore")] {
        if let Some(rows) = decoded_rows(&data[field]) {
            usable |= rows
                .as_object()
                .unwrap()
                .values()
                .any(|row| has_item_ids(&row[items]));
        }
    }
    usable |= data["itemshoes"].as_str().is_some_and(|rows| {
        rows.split('#').any(|row| {
            row.split('$')
                .next()
                .is_some_and(|ids| ids.split([',', '&']).any(|id| positive_id(id).is_some()))
        })
    });
    if !usable {
        return Err("Tencent has no Mayhem build for this champion yet.".into());
    }
    Ok(day)
}

fn rankings_for(payload: &Value, expected: &str) -> Option<Value> {
    let data = tencent_payload(payload, "listcollect")?;
    let returned = data.get("dtstatdate").filter(|value| !value.is_null());
    if let Some(date) = returned {
        let text = date
            .as_str()
            .map_or_else(|| date.to_string(), str::to_owned);
        if text != expected {
            return None;
        }
    }
    let list = data["listcollect"]
        .as_str()
        .filter(|value| !value.is_empty())?;
    let value = list
        .split('#')
        .map(|row| row.split('_').take(5).collect::<Vec<_>>().join("_"))
        .collect::<Vec<_>>()
        .join("#");
    let mut result = json!({"listcollect":value});
    if returned.is_some() {
        result["dtstatdate"] = expected.into();
    }
    Some(result)
}
pub fn read(root: &Path, id: u64, force: bool, fixture: bool) -> Result<Value, String> {
    if !(1..=10000).contains(&id) {
        return Err("Choose a valid champion".into());
    }
    let path = root.join(format!("mayhem-{id}.json"));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let cached = fs::metadata(&path)
        .ok()
        .filter(|m| m.len() < MAX_CACHE)
        .and_then(|_| serde_json::from_slice::<Value>(&fs::read(&path).ok()?).ok())
        .and_then(|mut saved| {
            let fetched = saved["build"]["fetchedAt"].as_u64()?;
            if saved["championId"] != id || now.checked_sub(fetched)? > 7 * DAY {
                return None;
            }
            let day = build_date(&saved["build"]["value"], id).ok()?;
            saved["rankings"] = rankings_for(&saved["rankings"]["value"], &day)
                .map(|value| json!({"value":value,"stale":false}))
                .unwrap_or(Value::Null);
            saved["rankingDate"] = day.into();
            Some(saved)
        });
    if !fixture
        && !force
        && cached
            .as_ref()
            .is_some_and(|s| now - s["build"]["fetchedAt"].as_u64().unwrap() < 900)
    {
        return Ok(cached.unwrap());
    }
    let fetch = || -> Result<Value, String> {
        let (build, rankings) = if fixture {
            if id != 1 {
                return Err("The recorded guide is for Annie".into());
            }
            (
                serde_json::from_str(include_str!("../../tests/fixtures/mayhem-annie.json"))
                    .unwrap(),
                Some(
                    serde_json::from_str(include_str!("../../tests/fixtures/mayhem-rankings.json"))
                        .unwrap(),
                ),
            )
        } else {
            let http = reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(12))
                .build()
                .map_err(|e| e.to_string())?;
            let get = |suffix: String| -> Result<Value, String> {
                let url = format!("https://mlol.qt.qq.com/go/battle_info/odp_proxy/{suffix}");
                capped_json(
                    http.get(url).send().map_err(|_| "Tencent is unavailable")?,
                    6 * 1024 * 1024,
                )
            };
            let build = get(format!("fuwen_hero_rank?championid={id}"))?;
            let rankings = get(format!(
                "fuwen_aram_hero_rank_v2?dtstatdate={}",
                build_date(&build, id)?
            ))
            .ok();
            (build, rankings)
        };
        let day = build_date(&build, id)?;
        let rankings = rankings.and_then(|value| rankings_for(&value, &day));
        let notices = if rankings.is_none() {
            vec!["Champion rankings are temporarily unavailable."]
        } else {
            vec![]
        };
        Ok(
            json!({"championId":id,"rankingDate":day,"notices":notices,"build":{"fetchedAt":now,"stale":false,"value":build},
            "rankings":rankings.map(|v|json!({"value":v,"stale":false}))}),
        )
    };
    match fetch() {
        Ok(value) => {
            if !fixture {
                let _ = save(&path, &value);
            }
            Ok(value)
        }
        Err(error) => cached
            .map(|mut s| {
                s["build"]["stale"] = true.into();
                if s["rankings"].is_object() {
                    s["rankings"]["stale"] = true.into();
                }
                s
            })
            .ok_or(error),
    }
}
#[test]
fn guide_rejects_a_date_without_build_data_and_rankings_for_another_day() {
    let build = json!({"dtstatdate":"20261120"});
    assert!(date(&build).is_err());
    let ranks = json!({"dtstatdate":"20261119","listcollect":"1_a_b_c_d_extra"});
    assert!(rankings_for(&ranks, "20261120").is_none());
    assert_eq!(
        rankings_for(&ranks, "20261119").unwrap()["listcollect"],
        "1_a_b_c_d"
    );

    let root = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    save(&root.path().join("mayhem-1.json"), &json!({
        "championId":1,"rankingDate":"20261119",
        "build":{"fetchedAt":now,"value":{"dtstatdate":"20261120","itemone_json":"{\"1\":{\"itemone\":\"1001\"}}"}},
        "rankings":{"stale":false,"value":ranks}
    })).unwrap();
    let loaded = read(root.path(), 1, false, false).unwrap();
    assert_eq!(loaded["rankingDate"], "20261120");
    assert!(loaded["rankings"].is_null());
}

#[test]
fn mayhem_catalogue_prefers_local_data_and_keeps_the_last_good_list() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("items.json");
    let url = "https://fixture.invalid/items.json";
    let list = Value::Array(vec![json!({"id":1}); 100]);
    let no_fetch = |_: &str| panic!("catalogue should use available local or fresh cached data");
    assert_eq!(
        catalogue_source(&file, url, 0, Some(list.clone()), &no_fetch).unwrap(),
        (list.clone(), false)
    );
    assert_eq!(
        catalogue_source(&file, url, 0, None, &|_| Ok(list.clone())).unwrap(),
        (list.clone(), false)
    );
    assert_eq!(
        catalogue_source(&file, url, DAY - 1, None, &no_fetch).unwrap(),
        (list.clone(), false)
    );
    let incomplete = |_: &str| Ok(json!([]));
    assert_eq!(
        catalogue_source(&file, url, DAY, None, &incomplete).unwrap(),
        (list, true)
    );
    assert!(catalogue_source(&file, url, 8 * DAY, None, &incomplete).is_err());
}

#[test]
fn recorded_guide_has_champion_and_date() {
    let d = tempfile::tempdir().unwrap();
    let v = read(d.path(), 1, false, true).unwrap();
    assert_eq!(v["championId"], 1);
    assert_eq!(v["rankingDate"].as_str().unwrap().len(), 8);
    assert!(read(d.path(), 0, false, true).is_err());
}

#[test]
fn mayhem_cache_accepts_only_renderable_builds_for_the_requested_champion() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/mayhem-annie.json")).unwrap();
    assert!(build_date(&fixture, 1).is_ok());
    assert!(build_date(&fixture, 103).is_err());
    assert!(build_date(&json!({"dtstatdate":"20261120","itemone_json":"{}"}), 1).is_err());
    assert!(build_date(
        &json!({"dtstatdate":"20260230","itemone_json":"{\"1\":{\"itemone\":\"1001\"}}"}),
        1
    )
    .is_err());
    assert!(build_date(
        &json!({"code":-1,"dtstatdate":"20261120","itemshoes":"1001$0.1$0.5"}),
        1
    )
    .is_err());
    assert!(build_date(
        &json!({"dtstatdate":"20261120","itemshoes":"1001$0.1$0.5"}),
        1
    )
    .is_ok());
    assert!(valid_date("20240229"));
    assert!(!valid_date("20260229"));
}
