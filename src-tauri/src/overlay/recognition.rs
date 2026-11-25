use super::data::{key, Catalogue};

#[derive(Clone, Debug)]
pub struct Text {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: u64,
    pub x: f32,
    pub y: f32,
}

pub fn detect(
    lines: &[Text],
    catalogue: &Catalogue,
    width: f32,
    height: f32,
) -> Option<[Choice; 3]> {
    if width < 640.0 || height < 400.0 {
        return None;
    }
    let prompt = lines.iter().any(|l| {
        let s = key(&l.text);
        [
            "chooseanaugment",
            "selectanaugment",
            "chooseyouraugment",
            "selectyouraugment",
            "chooseaugment",
            "selectaugment",
        ]
        .iter()
        .any(|p| s.contains(p))
    });
    if !prompt {
        return None;
    }
    let mut candidates = Vec::new();
    let titles: Vec<_> = lines
        .iter()
        .filter(|l| {
            l.y >= height * 0.15
                && l.y <= height * 0.72
                && l.h >= height * 0.012
                && l.text.len() <= 80
        })
        .collect();
    if titles.len() > 1200 {
        return None;
    }
    for line in &titles {
        if let Some(id) = catalogue.resolve(&line.text, line.h >= height * 0.018) {
            candidates.push((id, (*line).clone()));
        }
        // Titles can wrap, but do not combine unrelated columns or distant body text.
        for next in &titles {
            if next.y > line.y + line.h * 0.8
                && next.y < line.y + line.h * 2.4
                && (next.x + next.w / 2.0 - line.x - line.w / 2.0).abs() < width * 0.05
            {
                if let Some(id) = catalogue.resolve(
                    &format!("{} {}", line.text, next.text),
                    line.h >= height * 0.018,
                ) {
                    candidates.push((
                        id,
                        Text {
                            text: format!("{} {}", line.text, next.text),
                            x: line.x.min(next.x),
                            y: line.y,
                            w: (line.x + line.w).max(next.x + next.w) - line.x.min(next.x),
                            h: next.y + next.h - line.y,
                        },
                    ));
                }
            }
        }
    }
    // A full title wins over its own word fragments (for example, "Ultimate
    // Recursion" contains the different augment "Recursion"). Do not resolve
    // unrelated overlapping names: those must still fail as ambiguous.
    let contained: Vec<_> = candidates
        .iter()
        .map(|(id, t)| {
            candidates.iter().any(|(other, full)| {
                id != other
                    && full.w * full.h > t.w * t.h * 1.05
                    && t.x >= full.x - 2.
                    && t.y >= full.y - 2.
                    && t.x + t.w <= full.x + full.w + 2.
                    && t.y + t.h <= full.y + full.h + 2.
                    && key(&full.text).contains(&key(&t.text))
            })
        })
        .collect();
    let mut i = 0;
    candidates.retain(|_| {
        let keep = !contained[i];
        i += 1;
        keep
    });
    candidates.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
    candidates.dedup_by(|a, b| a.0 == b.0 && (a.1.x - b.1.x).abs() < width * 0.06);
    if candidates.len() > 12 {
        return None;
    }
    let mut triples = Vec::new();
    for a in &candidates {
        for b in &candidates {
            for c in &candidates {
                let ax = a.1.x + a.1.w / 2.0;
                let bx = b.1.x + b.1.w / 2.0;
                let cx = c.1.x + c.1.w / 2.0;
                if a.0 == b.0
                    || b.0 == c.0
                    || a.0 == c.0
                    || ax < width * 0.1
                    || cx > width * 0.9
                    || bx - ax < width * 0.09
                    || cx - bx < width * 0.09
                    || (bx - ax) / (cx - bx) > 1.35
                    || (cx - bx) / (bx - ax) > 1.35
                    || (a.1.y - b.1.y).abs() > height * 0.045
                    || (a.1.y - c.1.y).abs() > height * 0.045
                {
                    continue;
                }
                let selected = [a, b, c].map(|(id, title)| {
                    let center = title.x + title.w / 2.0;
                    let anchor = lines
                        .iter()
                        .filter(|l| {
                            let s = key(&l.text);
                            (s == "reroll" || s == "select")
                                && l.y > title.y + title.h + height * 0.05
                                && (l.x + l.w / 2.0 - center).abs() < width * 0.10
                        })
                        .min_by(|a, b| a.y.total_cmp(&b.y));
                    let y = anchor
                        .map(|a| a.y - height * 0.065)
                        .unwrap_or(title.y + height * 0.29)
                        .clamp(title.y + title.h + height * 0.025, height * 0.87);
                    Choice {
                        id: *id,
                        x: center,
                        y,
                    }
                });
                triples.push(selected);
            }
        }
    }
    triples.sort_by_key(|t| (t[0].id, t[1].id, t[2].id));
    triples.dedup_by(|a, b| a.iter().zip(b.iter()).all(|(a, b)| a.id == b.id));
    (triples.len() == 1).then(|| triples.remove(0))
}

#[derive(Default)]
pub struct Stable {
    last: Option<[Choice; 3]>,
    count: u8,
}
impl Stable {
    pub fn reset(&mut self) {
        self.last = None;
        self.count = 0;
    }
    pub fn accept(&mut self, choices: Option<[Choice; 3]>) -> Option<[Choice; 3]> {
        let Some(choices) = choices else {
            self.reset();
            return None;
        };
        if self.last.as_ref().is_some_and(|old| {
            old.iter()
                .zip(&choices)
                .all(|(a, b)| a.id == b.id && (a.x - b.x).abs() < 15.0 && (a.y - b.y).abs() < 15.0)
        }) {
            self.count = self.count.saturating_add(1);
        } else {
            self.count = 1;
        }
        self.last = Some(choices.clone());
        (self.count >= 2).then_some(choices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str, x: f32, y: f32) -> Text {
        Text {
            text: text.into(),
            x,
            y,
            w: 160.0,
            h: 25.0,
        }
    }
    #[test]
    fn requires_selection_prompt_three_distinct_aligned_names_and_stability() {
        let c = Catalogue::bundled();
        let mut lines = vec![
            line("Choose an Augment", 400.0, 50.0),
            line("Ultimate Recursion", 100.0, 210.0),
            line("ADAPt", 420.0, 210.0),
            line("Twin Fire", 740.0, 210.0),
        ];
        let found = detect(&lines, &c, 1000.0, 700.0).unwrap();
        assert_eq!(found.clone().map(|v| v.id), [3055, 1205, 1415]);
        let mut stable = Stable::default();
        assert!(stable.accept(Some(found.clone())).is_none());
        assert!(stable.accept(Some(found)).is_some());
        assert!(stable.accept(None).is_none());
        lines.remove(0);
        assert!(detect(&lines, &c, 1000.0, 700.0).is_none());
        lines.insert(0, line("Choose an Augment", 400.0, 50.0));
        lines[3].text = "ADAPt".into();
        assert!(detect(&lines, &c, 1000.0, 700.0).is_none());
    }
    #[test]
    fn reroll_hides_old_labels_and_unknown_cards_fail_closed() {
        let mut stable = Stable::default();
        let a = [
            Choice {
                id: 1,
                x: 1.0,
                y: 1.0,
            },
            Choice {
                id: 2,
                x: 2.0,
                y: 1.0,
            },
            Choice {
                id: 3,
                x: 3.0,
                y: 1.0,
            },
        ];
        stable.accept(Some(a.clone()));
        assert!(stable.accept(Some(a.clone())).is_some());
        let mut b = a;
        b[0].id = 4;
        assert!(stable.accept(Some(b)).is_none());
    }
    #[test]
    fn wrapped_titles_scale_and_ambiguous_offers_stay_hidden() {
        let c = Catalogue::bundled();
        let mut lines = vec![
            line("Choose an Augment", 400., 50.),
            line("Ultimate", 100., 210.),
            line("Recursion", 100., 240.),
            line("ADAPt", 420., 210.),
            line("Twin Fire", 740., 210.),
        ];
        assert_eq!(
            detect(&lines, &c, 1000., 700.).unwrap().map(|c| c.id),
            [3055, 1205, 1415]
        );
        for l in &mut lines {
            l.x *= 1.5;
            l.y *= 1.5;
            l.w *= 1.5;
            l.h *= 1.5;
        }
        assert_eq!(
            detect(&lines, &c, 1500., 1050.).unwrap().map(|c| c.id),
            [3055, 1205, 1415]
        );
        let mut extra = line("Homeguard", 100., 210.);
        extra.x *= 1.5;
        extra.y *= 1.5;
        extra.w *= 1.5;
        extra.h *= 1.5;
        lines.push(extra);
        assert!(detect(&lines, &c, 1500., 1050.).is_none());
    }
}
