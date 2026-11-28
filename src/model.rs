use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Lipid {
    pub id: String,
    pub name: String,
    pub family: String,
    pub subfamily: String,
    #[serde(default)]
    pub chemical_subfamily: String,
    #[serde(default)]
    pub group: String,
    pub area_nm2: f64,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mutation {
    pub residue: u64,
    pub target: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ptm {
    pub residue: u64,
    #[serde(flatten)]
    pub modification: Modification,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Glycan {
    pub iupac: String,
    #[serde(default)]
    pub root_anomer: String,
}
impl Default for Glycan {
    fn default() -> Self {
        Self {
            iupac: "GlcNAc".into(),
            root_anomer: "b".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Modification {
    Phosphorylation { charge: i32 },
    Palmitoylation,
    GlycineMyristoylation,
    LysineMyristoylation,
    Farnesylation,
    Geranylgeranylation,
    Triacylation,
    Glycosylation(Glycan),
}
impl Modification {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Phosphorylation { .. } => "phosphorylation",
            Self::Palmitoylation => "palmitoylation",
            Self::GlycineMyristoylation => "glycine_myristoylation",
            Self::LysineMyristoylation => "lysine_myristoylation",
            Self::Farnesylation => "farnesylation",
            Self::Geranylgeranylation => "geranylgeranylation",
            Self::Triacylation => "triacylation",
            Self::Glycosylation(_) => "glycosylation",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Phosphorylation { .. } => "Phosphorylation",
            Self::Palmitoylation => "S-palmitoylation",
            Self::GlycineMyristoylation => "N-myristoylation",
            Self::LysineMyristoylation => "Lys myristoylation",
            Self::Farnesylation => "S-farnesylation",
            Self::Geranylgeranylation => "S-geranylgeranylation",
            Self::Triacylation => "Triacylation",
            Self::Glycosylation(_) => "Glycosylation",
        }
    }
    pub fn uses_cysteine_sulfur(&self) -> bool {
        matches!(
            self,
            Self::Palmitoylation
                | Self::Farnesylation
                | Self::Geranylgeranylation
                | Self::Triacylation
        )
    }
    pub fn choices() -> [(Self, &'static str); 8] {
        [
            (Self::Phosphorylation { charge: -2 }, "Phosphorylation"),
            (Self::Palmitoylation, "Lipidation: Cys S-palmitoylation"),
            (
                Self::GlycineMyristoylation,
                "Lipidation: N-terminal Gly myristoylation",
            ),
            (Self::LysineMyristoylation, "Lipidation: Lys myristoylation"),
            (Self::Farnesylation, "Lipidation: Cys farnesylation"),
            (
                Self::Geranylgeranylation,
                "Lipidation: Cys geranylgeranylation",
            ),
            (
                Self::Triacylation,
                "Lipidation: N-terminal Cys triacylation",
            ),
            (Self::Glycosylation(Glycan::default()), "Glycosylation"),
        ]
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Protein {
    pub orient: bool,
    pub mutations: Vec<Mutation>,
    #[serde(serialize_with = "serialize_ptms")]
    pub ptms: Vec<Ptm>,
    pub ss_bonds: Vec<[usize; 2]>,
}
// Preserve the field order of earlier checkout payloads because their hash identifies a pending order.
fn serialize_ptms<S: serde::Serializer>(ptms: &[Ptm], serializer: S) -> Result<S::Ok, S::Error> {
    serde_json::to_value(ptms)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
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
                waters_per_lipid: 50.,
                clearance_nm: 1.,
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
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Residue {
    pub id: u64,
    pub name: String,
    pub number: i64,
    pub chain: String,
    #[serde(default)]
    pub n_terminal: bool,
    #[serde(default)]
    pub protein: bool,
    #[serde(default)]
    pub coarse_grained: bool,
    pub preview_index: Option<u64>,
    #[serde(skip)]
    pub duplicate_number: bool,
}
impl Residue {
    pub fn label(&self) -> String {
        let chain = self.chain.trim();
        let chain = if chain.is_empty() {
            String::new()
        } else {
            format!("{chain}:")
        };
        let mut label = format!("{} {chain}{}", self.name, self.number);
        if self.duplicate_number {
            label.push_str(&format!(" (resindex {})", self.id));
        }
        label
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disulfide {
    pub residues: [usize; 2],
    pub distance_nm: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProteinInventory {
    pub residues: Vec<Residue>,
    #[serde(default)]
    pub ptms: Vec<Residue>,
    #[serde(default)]
    pub ss_bonds: Vec<Disulfide>,
    pub disulfide_range_nm: [f64; 2],
    #[serde(default)]
    pub coarse_grained: bool,
    pub protein_selection: Option<String>,
}
impl ProteinInventory {
    pub fn mark_duplicate_numbers(&mut self) {
        let mut counts = std::collections::HashMap::new();
        for residue in &self.residues {
            *counts
                .entry((residue.chain.clone(), residue.number))
                .or_insert(0) += 1;
        }
        for residue in self.residues.iter_mut().chain(&mut self.ptms) {
            residue.duplicate_number = counts
                .get(&(residue.chain.clone(), residue.number))
                .is_some_and(|&count| count > 1);
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Partition {
    pub id: String,
    pub protein: Option<ProteinInventory>,
    pub ligands: Vec<String>,
    #[serde(default)]
    pub ligand_details: Vec<serde_json::Value>,
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
        if self.rows.is_empty() {
            return Err("Add at least one lipid species.".into());
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
        for (side, has_weight) in [
            ("outer", self.rows.iter().any(|r| r.upper > 0.)),
            ("inner", self.rows.iter().any(|r| r.lower > 0.)),
        ] {
            if !has_weight {
                return Err(format!("Add a positive percentage to the {side} leaflet."));
            }
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
            let weights = distribute(&values, 100000);
            let (leaflet_pairs, _) = weights.as_chunks::<2>();
            for (row, &[upper, lower]) in rows.iter_mut().zip(leaflet_pairs) {
                row.upper = upper as f64 / 1000.;
                row.lower = lower as f64 / 1000.;
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
    pub fn estimates(
        &self,
        catalogue: &[Lipid],
        geometry: &Value,
    ) -> Result<Vec<(Row, u32, u32)>, String> {
        let rows = self.normalized(catalogue)?;
        let areas: Vec<_> = rows
            .iter()
            .map(|r| catalogue.iter().find(|l| l.id == r.id).unwrap().area_nm2 as f32)
            .collect();
        let size = match geometry["xy_diameter_nm"].as_f64() {
            Some(diameter) => self
                .size_nm
                .max(((diameter + 2. * self.margin_nm) / (3f64.sqrt() / 2.) * 10.).ceil() / 10.)
                .max(geometry["minimum_cell_size_nm"].as_f64().unwrap_or(0.)),
            None => self.size_nm,
        } as f32;
        let area = size * (size * 120f32.to_radians().sin());
        let free =
            |side: &str| area - geometry["protein_areas"][side].as_f64().unwrap_or(0.) as f32;
        if free("upper") <= 0. || free("lower") <= 0. {
            return Err("The protein occupies the entire cell area in a leaflet.".into());
        }
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
            (
                allocate(upper, free("upper")),
                allocate(lower, free("lower")),
            )
        } else {
            let mut all = allocate([upper, lower].concat(), free("upper") + free("lower"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn glycan_checkout_keeps_previous_session_key_input() {
        let protein = Protein {
            ptms: vec![Ptm {
                residue: 42,
                modification: Modification::Glycosylation(Glycan::default()),
            }],
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&protein).unwrap(),
            r#"{"orient":false,"mutations":[],"ptms":[{"iupac":"GlcNAc","residue":42,"root_anomer":"b","type":"glycosylation"}],"ss_bonds":[]}"#
        );
    }

    #[test]
    fn protein_edits_keep_the_server_request_format() {
        let mutation = Mutation {
            residue: 12,
            target: "SER".into(),
        };
        assert_eq!(
            serde_json::to_value(mutation).unwrap(),
            json!({"residue":12,"target":"SER"})
        );
        for (modification, _) in Modification::choices() {
            let mut expected = json!({"residue":12,"type":modification.key()});
            match &modification {
                Modification::Glycosylation(_) => {
                    expected["iupac"] = json!("GlcNAc");
                    expected["root_anomer"] = json!("b");
                }
                Modification::Phosphorylation { .. } => expected["charge"] = json!(-2),
                _ => (),
            }
            let ptm = Ptm {
                residue: 12,
                modification,
            };
            assert_eq!(serde_json::to_value(&ptm).unwrap(), expected);
            let parsed: Ptm = serde_json::from_value(expected).unwrap();
            assert_eq!(parsed.modification, ptm.modification);
        }
    }
}
