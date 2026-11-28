use super::*;
mod chart;
mod composition;
mod picker;
use chart::LeafletChart;
use composition::{Composition, count_label, percent};
use picker::{LipidPicker, PickerState};

fn update(ui: Ui, f: impl FnOnce(&mut Membrane)) {
    ui.plan.update(|p| {
        if let Some(m) = &mut p.membrane {
            m.balancing_lipid = None;
            f(m);
        }
    });
    ui.confirmed.update(|v| {
        v[Page::Membrane as usize] = false;
        v[Page::Submit as usize] = false;
    });
}
fn row(ui: Ui, row: Row, state: PickerState, composition: Memo<Composition>) -> impl IntoView {
    let l = ui
        .catalog
        .with_untracked(|c| c.iter().find(|l| l.id == row.id).cloned().unwrap());
    let lipid = StoredValue::new(l.clone());
    let key = StoredValue::new(row.id.clone());
    let name_ref = NodeRef::<leptos::html::Button>::new();
    let inner = NodeRef::<leptos::html::Input>::new();
    if state
        .focus_row
        .with_value(|id| id.as_ref() == Some(&row.id))
    {
        state.focus_row.set_value(None);
        request_animation_frame(move || {
            if let Some(input) = inner.get_untracked() {
                let _ = input.focus();
            }
        });
    }
    view! {
        <tr>
            <th scope="row">
                <button
                    node_ref=name_ref
                    id=format!("{}-name", l.id)
                    class="lipid-name"
                    aria-label=format!("{}: show chemical structure", l.name)
                    on:mouseenter=move |_| state.show_hover(lipid.get_value(), name_ref)
                    on:focus=move |_| state.show_hover(lipid.get_value(), name_ref)
                    on:click=move |_| state.show_hover(lipid.get_value(), name_ref)
                    on:mouseleave=move |_| state.hover.set(None)
                    on:blur=move |_| state.hover.set(None)
                >
                    {l.name.clone()}
                </button>
                {(!l.tails.is_empty())
                    .then(|| view! { <span class="small lipid-tail">{l.tails.clone()}</span> })}
            </th>
            {["inner", "outer"]
                .into_iter()
                .enumerate()
                .map(|(i, side)| {
                    let id = format!("{}-{side}", row.id);
                    let input = if i == 0 { inner } else { NodeRef::new() };
                    let values = move || {
                        composition
                            .with(|c| {
                                let leaflet = &c.leaflets[i];
                                let name = key.get_value();
                                let j = leaflet.counts.iter().position(|(n, _, _)| *n == name);
                                let pct = j
                                    .and_then(|j| leaflet.weights[j])
                                    .map(percent)
                                    .unwrap_or_else(|| "—".into());
                                let count = count_label(
                                    j.filter(|_| c.valid).map(|j| leaflet.counts[j].1),
                                );
                                let delta = if ui
                                    .plan
                                    .with(|p| {
                                        p.membrane.as_ref().and_then(|m| m.balancing_lipid.as_ref())
                                            == Some(&name)
                                    })
                                {
                                    leaflet.adjustment
                                } else {
                                    0
                                };
                                (pct, count, delta)
                            })
                    };
                    view! {
                        <td>
                            <div class="percentage-count">
                                <span class="percentage-field">
                                    <input
                                        node_ref=input
                                        type="number"
                                        min="0"
                                        step="any"
                                        required
                                        value=if i == 0 { row.lower } else { row.upper }
                                        id=id.clone()
                                        aria-label=format!("{} {side} mole percentage", row.id)
                                        aria-describedby=format!(
                                            "{id}-normalized {id}-adjustment {id}-count",
                                        )
                                        on:input=move |e| {
                                            let n = event_target::<HtmlInputElement>(&e)
                                                .value_as_number();
                                            update(
                                                ui,
                                                |m| {
                                                    if let Some(r) = m
                                                        .rows
                                                        .iter_mut()
                                                        .find(|r| r.id == key.get_value())
                                                    {
                                                        if i == 0 {
                                                            r.lower = n;
                                                        } else {
                                                            r.upper = n;
                                                        }
                                                    }
                                                },
                                            );
                                        }
                                    />
                                </span>
                                <span
                                    class="normalized-percentage"
                                    id=format!("{id}-normalized")
                                    aria-label=move || {
                                        format!(
                                            "{} {side} normalized percentage: {}",
                                            key.get_value(),
                                            values().0,
                                        )
                                    }
                                >
                                    {move || values().0}
                                </span>
                                <span
                                    class="lipid-adjustment"
                                    hidden=move || values().2 == 0
                                    id=format!("{id}-adjustment")
                                >
                                    {move || format!("{:+}", values().2)}
                                </span>
                                <span class="lipid-count" id=format!("{id}-count")>
                                    {move || values().1}
                                </span>
                            </div>
                        </td>
                    }
                })
                .collect_view()}
            <td>
                <button
                    class="secondary remove-lipid"
                    aria-label=format!("Remove {}", row.id)
                    on:click=move |_| {
                        update(ui, |m| m.rows.retain(|r| r.id != key.get_value()));
                        state.hover.set(None);
                        if let Some(b) = state.button.get_untracked() {
                            let _ = b.focus();
                        }
                    }
                >
                    "×"
                </button>
            </td>
        </tr>
    }
}
#[component]
pub(super) fn MembranePage(ui: Ui) -> impl IntoView {
    let state = PickerState::new();
    let scroll = window_event_listener(leptos::ev::scroll, move |_| state.hover.set(None));
    let escape = window_event_listener(leptos::ev::keydown, move |e| {
        if e.key() == "Escape" {
            state.hover.set(None);
        }
    });
    on_cleanup(move || {
        scroll.remove();
        escape.remove();
    });
    let has_solute = move || ui.has_protein() || ui.has_ligands();
    let has_membrane = move || ui.plan.with(|p| p.membrane.is_some());
    let per_leaflet = move || {
        ui.plan
            .with(|p| p.membrane.as_ref().is_none_or(|m| m.per_leaflet))
    };
    let built = move || ui.confirmed.get()[Page::Submit as usize] && has_membrane();
    let composition = Memo::new(move |_| {
        ui.plan.with(|p| {
            p.membrane
                .as_ref()
                .map(|m| {
                    ui.catalog.with(|c| {
                        ui.result.with(|r| {
                            Composition::calculate(
                                m,
                                c,
                                r.as_ref()
                                    .filter(|_| ui.confirmed.get()[Page::Submit as usize])
                                    .map(|r| &r.geometry)
                                    .unwrap_or(&Value::Null),
                                has_solute(),
                            )
                        })
                    })
                })
                .unwrap_or_default()
        })
    });
    let error = move || {
        let msg = ui.msg.get();
        if msg.is_empty() {
            composition.with(|c| c.error.clone())
        } else {
            msg
        }
    };
    let balance_choice = RwSignal::new(String::new());
    let balance_options = move || {
        ui.plan.with(|p| {
            p.membrane
                .iter()
                .flat_map(|m| &m.rows)
                .filter(|r| r.upper + r.lower > 0.)
                .map(|r| r.id.clone())
                .collect::<Vec<_>>()
        })
    };
    let balance_id = move || {
        let options = balance_options();
        let choice = balance_choice.get();
        if options.contains(&choice) {
            Some(choice)
        } else {
            options.into_iter().next()
        }
    };
    let proceed = move |_| {
        ui.set_done(Page::Membrane, true);
        ui.go(Page::Solvent);
    };
    let toggle = move |e: leptos::ev::Event| {
        let checked = event_target_checked(&e);
        ui.plan.update(|p| {
            if checked {
                p.membrane = Some(ui.saved_membrane.get_value());
            } else if let Some(m) = p.membrane.take() {
                ui.saved_membrane.set_value(m);
            }
        });
        ui.set_done(Page::Membrane, false);
        ui.set_done(Page::Solvent, false);
        ui.set_done(Page::Submit, false);
    };
    view! {
        <StageHeading ui page=Page::Membrane />
        <label class="protein-check">
            <input
                id="add-membrane"
                type="checkbox"
                prop:checked=has_membrane
                disabled=move || ui.busy.get()
                on:change=toggle
            />
            " Add membrane"
        </label>
        <div id="no-membrane-actions" class="actions" hidden=has_membrane>
            <button
                id="continue-no-membrane"
                type="button"
                disabled=move || !ui.components_ready() || !has_solute() || ui.busy.get()
                on:click=proceed
            >
                "Continue"
            </button>
        </div>
        <p id="no-membrane-note" class="small" hidden=has_membrane>
            "The solute uses a rhombic dodecahedral box."
        </p>
        <div id="membrane-composition" class="composition-layout" hidden=move || !has_membrane()>
            <div class="composition-editor">
                <fieldset id="composition-controls" hidden=built disabled=move || ui.busy.get()>
                    <legend class="sr-only">"Membrane composition"</legend>
                    <p id="protein-area-note" class="small">
                        "During the build, the protein area is subtracted from each leaflet before lipid counts are calculated."
                    </p>
                    <p
                        id="composition-estimate"
                        class="small"
                        hidden=move || !composition.with(|c| c.estimated)
                    >
                        "Estimated counts and lipid areas from the entered cell size, without protein area."
                    </p>
                    <div class="composition-scroll">
                        <table class="composition">
                            <thead>
                                <tr>
                                    <th scope="col">"Lipid species"</th>
                                    <th scope="col">
                                        "Inner "
                                        <span class="small column-help">
                                            "Input → Normalized (lipids)"
                                        </span>
                                    </th>
                                    <th scope="col">
                                        "Outer "
                                        <span class="small column-help">
                                            "Input → Normalized (lipids)"
                                        </span>
                                    </th>
                                    <th scope="col">
                                        <span class="sr-only">"Remove"</span>
                                    </th>
                                </tr>
                            </thead>
                            <tbody id="lipid-rows">
                                {
                                    view! {
                                        <For
                                            each=move || {
                                                ui.plan
                                                    .with(|p| {
                                                        p.membrane
                                                            .as_ref()
                                                            .map(|m| m.rows.clone())
                                                            .unwrap_or_default()
                                                    })
                                            }
                                            key=|r| r.id.clone()
                                            children=move |r| row(ui, r, state, composition)
                                        />
                                    }
                                }
                            </tbody>
                            <tfoot>
                                <tr>
                                    <th scope="row">"Total"</th>
                                    <td>
                                        <div class="percentage-count">
                                            <span id="inner-total" class="normalized-percentage">
                                                {move || {
                                                    composition
                                                        .with(|c| {
                                                            c.leaflets[0]
                                                                .sum
                                                                .map(percent)
                                                                .unwrap_or_else(|| "—".into())
                                                        })
                                                }}
                                            </span>
                                            <span id="inner-total-count" class="lipid-count">
                                                {move || {
                                                    composition
                                                        .with(|c| count_label(
                                                            c
                                                                .valid
                                                                .then(|| c.leaflets[0].counts.iter().map(|v| v.1).sum()),
                                                        ))
                                                }}
                                            </span>
                                        </div>
                                    </td>
                                    <td>
                                        <div class="percentage-count">
                                            <span id="outer-total" class="normalized-percentage">
                                                {move || {
                                                    composition
                                                        .with(|c| {
                                                            c.leaflets[1]
                                                                .sum
                                                                .map(percent)
                                                                .unwrap_or_else(|| "—".into())
                                                        })
                                                }}
                                            </span>
                                            <span id="outer-total-count" class="lipid-count">
                                                {move || {
                                                    composition
                                                        .with(|c| count_label(
                                                            c
                                                                .valid
                                                                .then(|| c.leaflets[1].counts.iter().map(|v| v.1).sum()),
                                                        ))
                                                }}
                                            </span>
                                        </div>
                                    </td>
                                    <td></td>
                                </tr>
                                <tr id="area-row">
                                    <th scope="row">
                                        <div class="area-tools">
                                            <strong>"Area "<span class="small">"(nm²)"</span></strong>
                                            <div
                                                id="balance-controls"
                                                hidden=move || !composition.with(|c| c.balance_needed)
                                            >
                                                <label for="balancing-lipid" class="sr-only">
                                                    "Balancing lipid"
                                                </label>
                                                <select
                                                    id="balancing-lipid"
                                                    on:change=move |e| {
                                                        balance_choice.set(event_target_value(&e));
                                                        update(ui, |_| {});
                                                    }
                                                >
                                                    {move || {
                                                        balance_options()
                                                            .into_iter()
                                                            .map(|id| {
                                                                let selected = balance_id() == Some(id.clone());
                                                                let name = ui
                                                                    .catalog
                                                                    .with(|c| {
                                                                        c.iter()
                                                                            .find(|l| l.id == id)
                                                                            .map(|l| l.name.clone())
                                                                            .unwrap_or_else(|| id.clone())
                                                                    });
                                                                view! {
                                                                    <option value=id selected=selected>
                                                                        {name}
                                                                    </option>
                                                                }
                                                            })
                                                            .collect_view()
                                                    }}
                                                </select>
                                                <button
                                                    id="balance"
                                                    class="secondary"
                                                    type="button"
                                                    disabled=move || {
                                                        !composition.with(|c| c.valid)
                                                            || ui
                                                                .plan
                                                                .with(|p| {
                                                                    p.membrane
                                                                        .as_ref()
                                                                        .is_some_and(|m| m.balancing_lipid.is_some())
                                                                })
                                                    }
                                                    on:click=move |_| {
                                                        let id = balance_id();
                                                        update(ui, |m| m.balancing_lipid = id);
                                                    }
                                                >
                                                    "Balance"
                                                </button>
                                            </div>
                                        </div>
                                    </th>
                                    <td class="area-value">
                                        <span id="inner-area">
                                            {move || {
                                                composition
                                                    .with(|c| {
                                                        if c.valid {
                                                            format!("{:.3}", c.leaflets[0].area)
                                                        } else {
                                                            "—".into()
                                                        }
                                                    })
                                            }}
                                            {move || {
                                                composition
                                                    .with(|c| c.leaflets[0].excess)
                                                    .map(|x| {
                                                        view! {
                                                            <span class="area-excess">
                                                                {format!("(+{}%)", (x * 100.).round() / 100.)}
                                                            </span>
                                                        }
                                                    })
                                            }}
                                        </span>
                                    </td>
                                    <td class="area-value">
                                        <span id="outer-area">
                                            {move || {
                                                composition
                                                    .with(|c| {
                                                        if c.valid {
                                                            format!("{:.3}", c.leaflets[1].area)
                                                        } else {
                                                            "—".into()
                                                        }
                                                    })
                                            }}
                                            {move || {
                                                composition
                                                    .with(|c| c.leaflets[1].excess)
                                                    .map(|x| {
                                                        view! {
                                                            <span class="area-excess">
                                                                {format!("(+{}%)", (x * 100.).round() / 100.)}
                                                            </span>
                                                        }
                                                    })
                                            }}
                                        </span>
                                    </td>
                                    <td></td>
                                </tr>
                            </tfoot>
                        </table>
                    </div>
                    <button
                        id="add-lipid"
                        class="secondary"
                        type="button"
                        node_ref=state.button
                        disabled=move || ui.catalog.with(Vec::is_empty) || ui.busy.get()
                        on:click=move |_| state.open()
                    >
                        <span aria-hidden="true">"+"</span>
                        " Add lipids"
                    </button>
                    <fieldset class="normalization">
                        <legend class="sr-only">"Normalize percentages"</legend>
                        <div class="normalization-options">
                            <label>
                                <input
                                    id="per-leaflet"
                                    type="radio"
                                    name="normalization"
                                    value="leaflet"
                                    prop:checked=per_leaflet
                                    on:change=move |_| update(ui, |m| m.per_leaflet = true)
                                />
                                " 100% per leaflet"
                            </label>
                            <label>
                                <input
                                    id="per-membrane"
                                    type="radio"
                                    name="normalization"
                                    value="total"
                                    prop:checked=move || !per_leaflet()
                                    on:change=move |_| update(ui, |m| m.per_leaflet = false)
                                />
                                " 100% per membrane"
                            </label>
                        </div>
                        <p id="combined-total" class="small" hidden=per_leaflet>
                            {move || {
                                if composition.with(|c| c.leaflets[0].sum.is_some()) {
                                    "Normalized membrane total: 100%"
                                } else {
                                    "Normalized membrane total: —"
                                }
                            }}
                        </p>
                    </fieldset>
                    <div class="cell-size">
                        <label for="cell-size">"Hexagonal cell length (nm)"</label>
                        <input
                            id="cell-size"
                            type="number"
                            value="6"
                            min="3"
                            step="0.1"
                            required
                            on:input=move |e| update(
                                ui,
                                |m| {
                                    m.size_nm = event_target::<HtmlInputElement>(&e)
                                        .value_as_number();
                                },
                            )
                            max=move || (!has_solute()).then_some("12")
                        />
                    </div>
                    <div class="cell-size">
                        <label for="xy-margin">"Minimum XY margin (nm)"</label>
                        <input
                            id="xy-margin"
                            type="number"
                            value="1"
                            min="0"
                            step="0.1"
                            required
                            on:input=move |e| update(
                                ui,
                                |m| {
                                    m.margin_nm = event_target::<HtmlInputElement>(&e)
                                        .value_as_number();
                                },
                            )
                        />
                    </div>
                    <p id="resolved-cell" class="small">
                        {move || {
                            composition
                                .with(|c| c.resolved)
                                .map(|s| format!("Cell length after orientation: {s:.1} nm"))
                        }}
                    </p>
                    <div class="actions">
                        <button
                            id="build"
                            disabled=move || {
                                !composition.with(|c| c.valid) || !ui.components_ready()
                                    || ui.busy.get()
                            }
                            on:click=proceed
                        >
                            "Continue"
                        </button>
                    </div>
                </fieldset>
                <div id="computation" hidden>
                    <p id="compute-status" role="status">
                        "Building membrane…"
                    </p>
                    <progress id="compute-progress" aria-label="Building the membrane"></progress>
                </div>
                <div id="built-membrane" hidden=move || !built()>
                    <div class="actions">
                        <button
                            id="edit-membrane"
                            type="button"
                            on:click=move |_| {
                                ui.set_done(Page::Membrane, false);
                                ui.set_done(Page::Submit, false);
                            }
                        >
                            "Edit..."
                        </button>
                    </div>
                </div>
                <p
                    id="composition-error"
                    class="error"
                    role="alert"
                    hidden=move || error().is_empty()
                >
                    {error}
                </p>
                <p id="connection" class="error" role="status" hidden></p>
            </div>
            <aside class="leaflet-charts" aria-label="Lipid counts by leaflet">
                <LeafletChart side=0 composition />
                <LeafletChart side=1 composition />
            </aside>
        </div>
        <LipidPicker ui state />
        <aside
            id="lipid-hover"
            class="hover-structure"
            node_ref=state.popup
            hidden=move || state.hover.with(Option::is_none)
            style=move || {
                let (x, y) = state.position.get();
                format!("left:{x}px;top:{y}px")
            }
        >
            <strong>{move || state.hover.with(|l| l.as_ref().map(|l| l.name.clone()))}</strong>
            <img
                src=move || state.hover.with(|l| l.as_ref().map(|l| l.image.clone()))
                alt=move || {
                    state
                        .hover
                        .with(|l| l.as_ref().map(|l| format!("{} chemical structure", l.name)))
                }
            />
        </aside>
    }
}
