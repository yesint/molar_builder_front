use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lipid {
    pub id: String,
    pub name: String,
    pub family: String,
    pub subfamily: String,
    pub area_nm2: f32,
    pub image: String,
    pub tails: String,
}
#[derive(Clone, Deserialize)]
pub struct Catalogue {
    pub lipids: Vec<Lipid>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub id: String,
    pub lower: f64,
    pub upper: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Membrane {
    pub rows: Vec<Row>,
    pub per_leaflet: bool,
    pub size_nm: f64,
    pub margin_nm: f64,
    pub balancing_lipid: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solvent {
    pub waters_per_lipid: f64,
    pub clearance_nm: f64,
    pub salt: String,
    pub concentration: f64,
    pub neutralize: bool,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Protein {
    pub orient: bool,
    pub mutations: Vec<Value>,
    pub ptms: Vec<Value>,
    pub ss_bonds: Vec<[usize; 2]>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub partition: Option<String>,
    pub protein: Protein,
    pub edits: std::collections::BTreeMap<usize, String>,
    pub membrane: Option<Membrane>,
    pub solvent: Option<Solvent>,
    pub project: String,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            partition: None,
            protein: Protein {
                orient: true,
                ..Default::default()
            },
            edits: Default::default(),
            membrane: Some(Membrane {
                rows: vec![],
                per_leaflet: true,
                size_nm: 6.,
                margin_nm: 1.,
                balancing_lipid: None,
            }),
            solvent: Some(Solvent {
                waters_per_lipid: 40.,
                clearance_nm: 1.5,
                salt: "NaCl".into(),
                concentration: 0.15,
                neutralize: true,
            }),
            project: "project".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub id: String,
    pub protein_selection: String,
    pub ligand_selection: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Upload {
    pub id: String,
    pub name: String,
    pub format: String,
    pub report: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Partition {
    pub id: String,
    pub protein: Value,
    pub ligands: Vec<String>,
}

fn distribute(values: &[f64], units: u32) -> Vec<u32> {
    let max = values.iter().copied().fold(0., f64::max);
    let sum: f64 = values.iter().map(|v| v / max).sum();
    let exact: Vec<_> = values
        .iter()
        .map(|v| (v / max) / sum * units as f64)
        .collect();
    let mut result: Vec<_> = exact.iter().map(|v| v.floor() as u32).collect();
    let mut order: Vec<_> = (0..values.len()).collect();
    order.sort_by(|&i, &j| exact[j].fract().total_cmp(&exact[i].fract()));
    let missing = units - result.iter().sum::<u32>();
    for &i in order.iter().take(missing as usize) {
        result[i] += 1;
    }
    result
}
impl Membrane {
    pub fn normalized(&self, catalogue: &[Lipid]) -> Result<Vec<Row>, String> {
        if !self.size_nm.is_finite()
            || self.size_nm < 3.0
            || !self.margin_nm.is_finite()
            || self.margin_nm < 0.
        {
            return Err("Enter a cell length of at least 3 nm and a nonnegative XY margin.".into());
        }
        let mut seen = std::collections::HashSet::new();
        for r in &self.rows {
            if !catalogue.iter().any(|l| l.id == r.id)
                || !seen.insert(&r.id)
                || [r.upper, r.lower].iter().any(|x| !x.is_finite() || *x < 0.)
            {
                return Err(
                    "Choose distinct catalogue lipids and finite nonnegative weights.".into(),
                );
            }
        }
        if !self.rows.iter().any(|r| r.upper > 0.) || !self.rows.iter().any(|r| r.lower > 0.) {
            return Err("Both leaflets need a positive weight.".into());
        }
        let mut rows = self.rows.clone();
        if self.per_leaflet {
            for outer in [false, true] {
                let values: Vec<_> = rows
                    .iter()
                    .map(|r| if outer { r.upper } else { r.lower })
                    .collect();
                for (r, v) in rows.iter_mut().zip(distribute(&values, 100000)) {
                    if outer {
                        r.upper = v as f64 / 1000.
                    } else {
                        r.lower = v as f64 / 1000.
                    };
                }
            }
        } else {
            let values: Vec<_> = rows.iter().flat_map(|r| [r.upper, r.lower]).collect();
            for (r, v) in rows
                .iter_mut()
                .zip(distribute(&values, 100000).chunks_exact(2))
            {
                r.upper = v[0] as f64 / 1000.;
                r.lower = v[1] as f64 / 1000.;
            }
        }
        rows.retain(|r| r.upper + r.lower > 0.);
        if !rows.iter().any(|r| r.upper > 0.) || !rows.iter().any(|r| r.lower > 0.) {
            return Err("A leaflet rounds to zero.".into());
        }
        if self
            .balancing_lipid
            .as_ref()
            .is_some_and(|id| !rows.iter().any(|r| r.id == *id))
        {
            return Err("Select a balancing lipid from the composition.".into());
        }
        Ok(rows)
    }
    pub fn estimates(&self, catalogue: &[Lipid]) -> Result<Vec<(Row, u32, u32)>, String> {
        let rows = self.normalized(catalogue)?;
        let areas: Vec<_> = rows
            .iter()
            .map(|r| catalogue.iter().find(|l| l.id == r.id).unwrap().area_nm2)
            .collect();
        let area = (self.size_nm as f32) * (self.size_nm as f32 * 120f32.to_radians().sin());
        let allocate = |weights: Vec<f32>, surface: f32| {
            let total: f32 = weights.iter().sum();
            let mean = weights
                .iter()
                .zip(areas.iter().cycle())
                .map(|(w, a)| w * a)
                .sum::<f32>()
                / total;
            let count = (surface / mean).round() as u32;
            let values: Vec<_> = weights.iter().map(|w| (w / total) * count as f32).collect();
            let mut out: Vec<_> = values.iter().map(|v| v.floor() as u32).collect();
            let mut order: Vec<_> = (0..values.len()).collect();
            order.sort_by(|&a, &b| values[b].fract().total_cmp(&values[a].fract()));
            for i in order
                .into_iter()
                .take(count.saturating_sub(out.iter().sum()) as usize)
            {
                out[i] += 1;
            }
            out
        };
        let upper = rows.iter().map(|r| r.upper as f32).collect::<Vec<_>>();
        let lower = rows.iter().map(|r| r.lower as f32).collect::<Vec<_>>();
        let (up, lo) = if self.per_leaflet {
            (allocate(upper, area), allocate(lower, area))
        } else {
            let mut all = allocate([upper, lower].concat(), 2. * area);
            let lo = all.split_off(rows.len());
            (all, lo)
        };
        Ok(rows
            .into_iter()
            .zip(lo)
            .zip(up)
            .map(|((r, l), u)| (r, l, u))
            .collect())
    }
}
impl Plan {
    pub fn validate(&self, catalogue: &[Lipid]) -> Result<(), String> {
        if self.partition.is_none() && self.membrane.is_none() {
            return Err("Select molecular components or add a membrane.".into());
        }
        if self.project.is_empty()
            || self.project.len() > 120
            || !self
                .project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" -_.".contains(&b))
            || !self.project.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err("Use a project name beginning with a letter or digit.".into());
        }
        if let Some(m) = &self.membrane {
            if self.partition.is_none() && m.size_nm > 12.0 {
                return Err("The maximum cell length for a membrane alone is 12 nm.".into());
            }
            m.normalized(catalogue)?;
        }
        if let Some(s) = &self.solvent
            && (!["NaCl", "KCl", "CaCl2", "MgCl2"].contains(&s.salt.as_str())
                || [s.clearance_nm, s.concentration, s.waters_per_lipid]
                    .iter()
                    .any(|v| !v.is_finite() || *v < 0.)
                || (self.membrane.is_some() && s.waters_per_lipid <= 0.))
        {
            return Err("Check the salt and solvent settings.".into());
        }
        Ok(())
    }
}
