use super::*;

pub(super) fn solvent(ui: Ui) -> impl IntoView {
    let init = ui
        .plan
        .get_untracked()
        .solvent
        .unwrap_or_else(|| ui.saved_solvent.get_value());
    view! {
        <label>
            <input
                type="checkbox"
                prop:checked=move || ui.plan.with(|p| p.solvent.is_some())
                on:change=move |e| {
                    ui.plan
                        .update(|p| {
                            p.solvent = if input(&e).checked() {
                                Some(ui.saved_solvent.get_value())
                            } else {
                                if let Some(settings) = p.solvent.take() {
                                    ui.saved_solvent.set_value(settings);
                                }
                                None
                            };
                        })
                }
            />
            "Add water and ions"
        </label>
        <section hidden=move || {
            ui.plan.with(|p| p.solvent.is_none())
        }>
            {[
                (0, "Waters per lipid", init.waters_per_lipid),
                (1, "Water clearance (nm)", init.clearance_nm),
                (2, "Salt concentration (mol/L)", init.concentration),
            ]
                .into_iter()
                .map(move |(i, name, v)| {
                    view! {
                        <label>
                            {name}
                            <input
                                type="number"
                                min="0"
                                step="any"
                                value=v
                                on:input=move |e| {
                                    ui.plan
                                        .update(|p| {
                                            if let Some(s) = &mut p.solvent {
                                                let v = event_target_value(&e).parse().unwrap_or(-1.);
                                                match i {
                                                    0 => s.waters_per_lipid = v,
                                                    1 => s.clearance_nm = v,
                                                    _ => s.concentration = v,
                                                }
                                            }
                                        })
                                }
                            />
                        </label>
                    }
                })
                .collect_view()}
            <label>
                "Salt"
                <select on:change=move |e| {
                    ui.plan
                        .update(|p| {
                            if let Some(s) = &mut p.solvent {
                                s.salt = event_target_value(&e);
                            }
                        })
                }>
                    {["NaCl", "KCl", "CaCl2", "MgCl2"]
                        .into_iter()
                        .map(|s| view! { <option selected=init.salt == s>{s}</option> })
                        .collect_view()}
                </select>
            </label> <label>
                <input
                    type="checkbox"
                    checked=init.neutralize
                    on:change=move |e| {
                        ui.plan
                            .update(|p| {
                                if let Some(s) = &mut p.solvent {
                                    s.neutralize = input(&e).checked();
                                }
                            })
                    }
                />
                "Neutralize net charge"
            </label>
        </section>
    }
}
