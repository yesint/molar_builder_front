use super::*;
pub(super) fn setup(ui: Ui) {
    on("assembly-options", "input", move |_| {
        ui.plan.update(|p| {
            if field("add-solvent").checked() {
                p.solvent = Some(Solvent {
                    waters_per_lipid: field("waters-per-lipid").value_as_number(),
                    clearance_nm: field("water-clearance").value_as_number(),
                    salt: select("assembly-salt").value(),
                    concentration: field("salt-concentration").value_as_number(),
                    neutralize: field("neutralize-system").checked(),
                });
            } else {
                p.solvent = None;
            }
        });
        ui.set_done(4, false);
        ui.set_done(5, false);
    });
    click("continue-solvent", move || {
        let p = ui.plan.get_untracked();
        if p.solvent.is_some()
            && ["waters-per-lipid", "water-clearance", "salt-concentration"]
                .iter()
                .any(|id| {
                    (*id != "waters-per-lipid" || p.membrane.is_some())
                        && !field(id).report_validity()
                })
        {
            return;
        }
        ui.set_done(4, true);
        ui.go(5);
    });
    Effect::new(move |_| {
        let p = ui.plan.get();
        let membrane = p.membrane.is_some();
        let solvent = p.solvent.is_some();
        visible("waters-per-lipid", membrane);
        visible("waters-per-lipid-label", membrane);
        visible("solvent-parameters", solvent);
        enabled("solvent-parameters", solvent);
        label(
            "assembly-box",
            if membrane {
                "Box: hexagonal membrane cell."
            } else {
                "Box: rhombic dodecahedron."
            },
        );
        enabled("continue-solvent", ui.ready() && !ui.busy.get());
        enabled("assembly-options", !ui.busy.get());
    });
}
