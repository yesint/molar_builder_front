use super::*;

pub(super) fn membrane(ui: Ui) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let family = RwSignal::new(String::new());
    let sub = RwSignal::new(String::new());
    let initial = ui
        .plan
        .get_untracked()
        .membrane
        .unwrap_or_else(|| ui.saved_membrane.get_value());
    view! {
        <label>
            <input
                type="checkbox"
                prop:checked=move || ui.plan.with(|p| p.membrane.is_some())
                on:change=move |e| {
                    ui.plan
                        .update(|p| {
                            p.membrane = if input(&e).checked() {
                                Some(ui.saved_membrane.get_value())
                            } else {
                                if let Some(settings) = p.membrane.take() {
                                    ui.saved_membrane.set_value(settings);
                                }
                                None
                            };
                        })
                }
            />
            "Add a membrane"
        </label>
        <section hidden=move || ui.plan.with(|p| p.membrane.is_none())>
            <label>
                "Cell length (nm)"
                <input
                    type="number"
                    min="3"
                    max=move || ui.plan.with(|p| p.partition.is_none().then_some("12"))
                    step="0.1"
                    value=initial.size_nm
                    on:input=move |e| update_membrane(
                        ui,
                        move |m| m.size_nm = event_target_value(&e).parse().unwrap_or(0.),
                    )
                />
            </label>
            <label>
                "Minimum XY margin (nm)"
                <input
                    type="number"
                    step="0.1"
                    min="0"
                    value=initial.margin_nm
                    on:input=move |e| update_membrane(
                        ui,
                        move |m| {
                            m.margin_nm = event_target_value(&e).parse().unwrap_or(-1.);
                        },
                    )
                />
            </label>
            <label>
                "Normalize weights"
                <select on:change=move |e| update_membrane(
                    ui,
                    move |m| m.per_leaflet = event_target_value(&e) == "leaflet",
                )>
                    <option value="leaflet" selected=initial.per_leaflet>
                        "Per leaflet"
                    </option>
                    <option value="total" selected=!initial.per_leaflet>
                        "Whole membrane"
                    </option>
                </select>
            </label>
            <h2>"Lipids"</h2>
            <label>
                "Family"
                <select on:change=move |e| {
                    family.set(event_target_value(&e));
                    sub.set(String::new())
                }>
                    <option value="">"All families"</option>
                    {move || {
                        ui.catalog
                            .with(|c| {
                                c.iter()
                                    .map(|l| l.family.clone())
                                    .collect::<std::collections::BTreeSet<_>>()
                                    .into_iter()
                                    .map(|s| view! { <option>{s}</option> })
                                    .collect_view()
                            })
                    }}
                </select>
            </label>
            <label>
                "Subfamily"<select on:change=move |e| sub.set(event_target_value(&e))>
                    <option value="">"All subfamilies"</option>
                    {move || {
                        ui.catalog
                            .with(|c| {
                                c.iter()
                                    .filter(|l| family.get().is_empty() || l.family == family.get())
                                    .map(|l| l.subfamily.clone())
                                    .collect::<std::collections::BTreeSet<_>>()
                                    .into_iter()
                                    .map(|s| view! { <option>{s}</option> })
                                    .collect_view()
                            })
                    }}
                </select>
            </label>
            <label>
                "Search by lipid or tail composition"
                <input
                    type="search"
                    on:input=move |e| query.set(event_target_value(&e).to_lowercase())
                />
            </label>
            <div class="picker">
                {move || {
                    let q = query.get();
                    let f = family.get();
                    let s = sub.get();
                    ui.catalog
                        .with(|c| {
                            c.iter()
                                .filter(|l| {
                                    (f.is_empty() || f == l.family)
                                        && (s.is_empty() || s == l.subfamily)
                                        && format!(
                                            "{} {} {} {} {}",
                                            l.id,
                                            l.name,
                                            l.tails,
                                            l.family,
                                            l.subfamily,
                                        )
                                            .to_lowercase()
                                            .contains(&q)
                                })
                                .map(|l| {
                                    let id = l.id.clone();
                                    let label = l.id.clone();
                                    let src = l.image.clone();
                                    view! {
                                        <button
                                            class="lipid"
                                            on:click=move |_| {
                                                ui.plan
                                                    .update(|p| {
                                                        if let Some(m) = &mut p.membrane
                                                            && !m.rows.iter().any(|r| r.id == id)
                                                        {
                                                            m.rows
                                                                .push(Row {
                                                                    id: id.clone(),
                                                                    upper: 0.,
                                                                    lower: 0.,
                                                                });
                                                        }
                                                    })
                                            }
                                        >
                                            {label}
                                            <img loading="lazy" src=src alt="Chemical structure" />
                                        </button>
                                    }
                                })
                                .collect_view()
                        })
                }}
            </div>
            <table>
                <thead>
                    <tr>
                        <th>"Species"</th>
                        <th>"Inner weight"</th>
                        <th>"Outer weight"</th>
                        <th>"Balance"</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    <For
                        each=move || {
                            ui.plan
                                .with(|p| {
                                    p.membrane.as_ref().map(|m| m.rows.clone()).unwrap_or_default()
                                })
                        }
                        key=|r| r.id.clone()
                        children=move |r| {
                            let id = StoredValue::new(r.id.clone());
                            view! {
                                <tr>
                                    <th>{r.id}</th>
                                    {[(false, r.lower), (true, r.upper)]
                                        .into_iter()
                                        .map(move |(upper, value)| {
                                            view! {
                                                <td>
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        step="any"
                                                        aria-label=if upper {
                                                            "Outer weight"
                                                        } else {
                                                            "Inner weight"
                                                        }
                                                        value=value
                                                        on:input=move |e| {
                                                            ui.plan
                                                                .update(|p| {
                                                                    if let Some(row) = p
                                                                        .membrane
                                                                        .as_mut()
                                                                        .and_then(|m| {
                                                                            m.rows.iter_mut().find(|r| r.id == id.get_value())
                                                                        })
                                                                    {
                                                                        let v = event_target_value(&e).parse().unwrap_or(0.);
                                                                        if upper { row.upper = v } else { row.lower = v }
                                                                    }
                                                                })
                                                        }
                                                    />
                                                </td>
                                            }
                                        })
                                        .collect_view()}
                                    <td>
                                        <input
                                            type="checkbox"
                                            aria-label="Balance using this species"
                                            prop:checked=move || {
                                                ui.plan
                                                    .with(|p| {
                                                        p.membrane.as_ref().and_then(|m| m.balancing_lipid.as_ref())
                                                            == Some(&id.get_value())
                                                    })
                                            }
                                            on:change=move |e| update_membrane(
                                                ui,
                                                move |m| {
                                                    m.balancing_lipid = input(&e)
                                                        .checked()
                                                        .then(|| id.get_value());
                                                },
                                            )
                                        />
                                    </td>
                                    <td>
                                        <button on:click=move |_| update_membrane(
                                            ui,
                                            move |m| {
                                                m.rows.retain(|r| r.id != id.get_value());
                                                if m.balancing_lipid == Some(id.get_value()) {
                                                    m.balancing_lipid = None;
                                                }
                                            },
                                        )>"Remove"</button>
                                    </td>
                                </tr>
                            }
                        }
                    />
                </tbody>
            </table>
            <p>
                {move || {
                    ui.plan
                        .with(|p| {
                            p.membrane
                                .as_ref()
                                .map(|m| match m.estimates(&ui.catalog.get()) {
                                    Ok(rows) => {
                                        rows.iter()
                                            .map(|(r, l, u)| {
                                                format!(
                                                    "{}: inner {}% ({l}), outer {}% ({u})",
                                                    r.id,
                                                    r.lower,
                                                    r.upper,
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                            .join(" · ")
                                    }
                                    Err(e) => e,
                                })
                                .unwrap_or_default()
                        })
                }}
            </p>
        </section>
    }
}

fn update_membrane(ui: Ui, update: impl FnOnce(&mut Membrane)) {
    ui.plan.update(|plan| {
        if let Some(settings) = &mut plan.membrane {
            update(settings);
        }
    });
}
