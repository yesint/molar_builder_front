use super::*;

#[derive(Clone, Default, PartialEq)]
pub(super) struct Leaflet {
    pub(super) weights: Vec<Option<f64>>,
    pub(super) counts: Vec<(String, u32, usize)>,
    pub(super) sum: Option<f64>,
    pub(super) area: f64,
    pub(super) excess: Option<f64>,
    pub(super) adjustment: i32,
}
#[derive(Clone, Default, PartialEq)]
pub(super) struct Composition {
    pub(super) valid: bool,
    pub(super) error: String,
    pub(super) leaflets: [Leaflet; 2],
    pub(super) resolved: Option<f64>,
    pub(super) estimated: bool,
    pub(super) balance_needed: bool,
}
impl Composition {
    pub(super) fn calculate(m: &Membrane, c: &[Lipid], geometry: &Value, has_solute: bool) -> Self {
        let normalized = m.normalized(c).unwrap_or_default();
        let tenth = |v: f64| (v * 10. - (v * 10.).round()).abs() < 1e-7;
        let estimates = m.estimates(c, geometry).and_then(|v| {
            if !tenth(m.size_nm) || (!has_solute && m.size_nm > 12.) {
                return Err("Enter a valid cell length, in steps of 0.1 nm.".into());
            }
            if !tenth(m.margin_nm) {
                return Err("Enter a finite, nonnegative XY margin.".into());
            }
            if !v.iter().any(|(_, lo, _)| *lo > 0) || !v.iter().any(|(_, _, up)| *up > 0) {
                return Err("The cell is too small for a lipid in each leaflet.".into());
            }
            Ok(v)
        });
        let mut error = estimates.as_ref().err().cloned().unwrap_or_default();
        let mut estimates = estimates.unwrap_or_default();
        let mut area = [0.; 2];
        for (r, l, u) in &estimates {
            let a = c.iter().find(|v| v.id == r.id).unwrap().area_nm2;
            area[0] += a * f64::from(*l);
            area[1] += a * f64::from(*u);
        }
        let protein_area =
            ["lower", "upper"].map(|side| geometry["protein_areas"][side].as_f64().unwrap_or(0.));
        let total_counts = estimates
            .iter()
            .fold([0, 0], |[lo, up], (_, l, u)| [lo + *l, up + *u]);
        let mut delta = [0i32; 2];
        if let Some(id) = &m.balancing_lipid
            && let Some((_, l, u)) = estimates.iter_mut().find(|(r, _, _)| r.id == *id)
        {
            let a = c.iter().find(|v| v.id == *id).unwrap().area_nm2;
            let larger = usize::from(area[1] + protein_area[1] > area[0] + protein_area[0]);
            let gap = (area[0] + protein_area[0] - area[1] - protein_area[1]).abs();
            let total = total_counts[larger] as i32;
            let take = ([*l, *u][larger] as i32).min(total - 1);
            let mut best = gap;
            for n in [(gap / a).floor() as i32, (gap / a).ceil() as i32, take] {
                if n <= 0 {
                    continue;
                }
                for side in [larger, 1 - larger] {
                    if side == larger && n > take {
                        continue;
                    }
                    let residual = (gap - a * f64::from(n)).abs();
                    if residual < best - 1e-9 {
                        best = residual;
                        delta = [0; 2];
                        delta[side] = if side == larger { -n } else { n };
                    }
                }
            }
            if delta == [0, 0] {
                error = "No whole-lipid adjustment can reduce the area difference.".into();
            }
            *l = l.saturating_add_signed(delta[0]);
            *u = u.saturating_add_signed(delta[1]);
            for i in 0..2 {
                area[i] += f64::from(delta[i]) * a;
            }
        }
        let estimated = has_solute && geometry.is_null();
        let resolved = geometry["xy_diameter_nm"].as_f64().map(|d| {
            m.size_nm
                .max(((d + 2. * m.margin_nm) / (3f64.sqrt() / 2.) * 10.).ceil() / 10.)
                .max(geometry["minimum_cell_size_nm"].as_f64().unwrap_or(0.))
        });
        let valid = !estimates.is_empty();
        let total = [area[0] + protein_area[0], area[1] + protein_area[1]];
        let smaller = total[0].min(total[1]);
        let mismatch = if smaller > 0. {
            100. * (total[0] - total[1]).abs() / smaller
        } else {
            0.
        };
        let leaflets = std::array::from_fn(|side| {
            let weights: Vec<_> = m
                .rows
                .iter()
                .map(|r| {
                    normalized
                        .iter()
                        .find(|n| n.id == r.id)
                        .map(|n| [n.lower, n.upper][side])
                        .or((!normalized.is_empty()).then_some(0.))
                })
                .collect();
            let sum = (!normalized.is_empty()).then(|| weights.iter().flatten().sum());
            let counts = m
                .rows
                .iter()
                .enumerate()
                .map(|(j, r)| {
                    let count = estimates
                        .iter()
                        .find(|(n, _, _)| n.id == r.id)
                        .map_or(0, |(_, l, u)| [*l, *u][side]);
                    (r.id.clone(), count, j)
                })
                .collect();
            Leaflet {
                weights,
                counts,
                sum,
                area: area[side],
                excess: (total[side] > total[1 - side]).then_some(mismatch),
                adjustment: delta[side],
            }
        });
        Self {
            valid,
            error: if m.rows.is_empty() {
                String::new()
            } else {
                error
            },
            leaflets,
            resolved,
            estimated: estimated && valid,
            balance_needed: valid && mismatch > 5.,
        }
    }
}
pub(super) fn percent(n: f64) -> String {
    format!("{}%", (n * 1000.).round() / 1000.)
}
pub(super) fn count_label(n: Option<u32>) -> String {
    n.map(|n| format!("({n})")).unwrap_or_else(|| "(—)".into())
}
