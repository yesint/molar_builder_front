use super::*;
#[component]
pub(super) fn SolventPage(ui: Ui) -> impl IntoView {
    let draft = RwSignal::new(
        ui.plan
            .with_untracked(|p| p.solvent.clone())
            .unwrap_or_else(|| Plan::default().solvent.unwrap()),
    );
    let enabled = move || ui.plan.with(|p| p.solvent.is_some());
    let membrane = move || ui.plan.with(|p| p.membrane.is_some());
    let invalidate = move || {
        ui.set_done(Page::Solvent, false);
        ui.set_done(Page::Submit, false);
    };
    let save = move || {
        ui.plan.update(|p| p.solvent = Some(draft.get_untracked()));
        invalidate();
    };
    let waters_per_lipid = NodeRef::<leptos::html::Input>::new();
    let clearance_nm = NodeRef::<leptos::html::Input>::new();
    let concentration = NodeRef::<leptos::html::Input>::new();
    let proceed = move |_| {
        if enabled()
            && [
                membrane().then_some(waters_per_lipid),
                Some(clearance_nm),
                Some(concentration),
            ]
            .into_iter()
            .flatten()
            .any(|input| {
                input
                    .get_untracked()
                    .is_none_or(|input| !input.report_validity())
            })
        {
            return;
        }
        ui.set_done(Page::Solvent, true);
        ui.go(Page::Submit);
    };
    view! {
        <StageHeading ui page=Page::Solvent />
        <fieldset id="assembly-options" disabled=move || ui.busy.get()>
            <label class="protein-check">
                <input
                    id="add-solvent"
                    type="checkbox"
                    prop:checked=enabled
                    on:change=move |e| {
                        ui.plan
                            .update(|p| {
                                p.solvent = event_target_checked(&e).then(|| draft.get_untracked());
                            });
                        invalidate();
                    }
                />
                " Add solvent"
            </label>
            <fieldset
                id="solvent-parameters"
                class="assembly-options"
                hidden=move || !enabled()
                disabled=move || !enabled()
            >
                <label
                    id="waters-per-lipid-label"
                    for="waters-per-lipid"
                    hidden=move || !membrane()
                >
                    "Water molecules per lipid"
                </label>
                <input
                    id="waters-per-lipid"
                    type="number"
                    min="1"
                    step="1"
                    value="50"
                    required
                    node_ref=waters_per_lipid
                    on:input=move |e| {
                        let n = event_target::<HtmlInputElement>(&e).value_as_number();
                        draft.update(|s| s.waters_per_lipid = n);
                        save();
                    }
                    hidden=move || !membrane()
                />
                <label for="water-clearance">"Water clearance beyond protein (nm)"</label>
                <input
                    id="water-clearance"
                    type="number"
                    min="0"
                    step="0.1"
                    value="1"
                    required
                    node_ref=clearance_nm
                    on:input=move |e| {
                        let n = event_target::<HtmlInputElement>(&e).value_as_number();
                        draft.update(|s| s.clearance_nm = n);
                        save();
                    }
                />
                <label for="assembly-salt">"Salt"</label>
                <select
                    id="assembly-salt"
                    on:change=move |e| {
                        draft.update(|s| s.salt = event_target_value(&e));
                        save();
                    }
                >
                    <option>"NaCl"</option>
                    <option>"KCl"</option>
                    <option>"CaCl2"</option>
                    <option>"MgCl2"</option>
                </select>
                <label for="salt-concentration">"Concentration (mol/L)"</label>
                <input
                    id="salt-concentration"
                    type="number"
                    min="0"
                    step="0.01"
                    value="0.15"
                    required
                    node_ref=concentration
                    on:input=move |e| {
                        let n = event_target::<HtmlInputElement>(&e).value_as_number();
                        draft.update(|s| s.concentration = n);
                        save();
                    }
                />
                <label class="protein-check">
                    <input
                        id="neutralize-system"
                        type="checkbox"
                        prop:checked=move || draft.with(|s| s.neutralize)
                        on:change=move |e| {
                            draft.update(|s| s.neutralize = event_target_checked(&e));
                            save();
                        }
                    />
                    " Neutralize"
                </label>
            </fieldset>
        </fieldset>
        <p id="assembly-box" class="small">
            {move || {
                if membrane() {
                    "Box: hexagonal membrane cell."
                } else {
                    "Box: rhombic dodecahedron."
                }
            }}
        </p>
        <div class="actions">
            <button
                id="continue-solvent"
                type="button"
                disabled=move || !ui.ready() || ui.busy.get()
                on:click=proceed
            >
                "Continue"
            </button>
        </div>
    }
}
