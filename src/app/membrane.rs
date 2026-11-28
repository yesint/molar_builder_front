use super::*;
use crate::viewer::PALETTE;

fn update(ui: Ui, f: impl FnOnce(&mut Membrane)) {
    ui.plan.update(|p| {
        if let Some(m) = &mut p.membrane {
            m.balancing_lipid = None;
            f(m);
        }
    });
    ui.confirmed.update(|v| {
        v[3] = false;
        v[5] = false;
    });
}
fn preview(l: &Lipid) {
    label("preview-name", &l.name);
    notice("preview-tails", &l.tails);
    visible("preview-content", true);
    visible("preview-image", true);
    visible("preview-error", false);
    let image = el("preview-image");
    image.set_attribute("src", &l.image).unwrap();
    image
        .set_attribute("alt", &format!("{} chemical structure", l.name))
        .unwrap();
}
fn hover(l: &Lipid) {
    let host = el("lipid-hover");
    host.query_selector("strong")
        .unwrap()
        .unwrap()
        .set_text_content(Some(&l.name));
    let img = host.query_selector("img").unwrap().unwrap();
    img.set_attribute("src", &l.image).unwrap();
    img.set_attribute("alt", &format!("{} chemical structure", l.name))
        .unwrap();
    visible("lipid-hover", true);
    let rect = el(&format!("{}-name", l.id)).get_bounding_client_rect();
    let size = host.get_bounding_client_rect();
    let x = (rect.right() + 12.)
        .min(window().inner_width().unwrap().as_f64().unwrap() - size.width() - 8.)
        .max(8.);
    let y = rect
        .top()
        .min(window().inner_height().unwrap().as_f64().unwrap() - size.height() - 8.)
        .max(8.);
    host.set_attribute("style", &format!("left:{x}px;top:{y}px"))
        .unwrap();
}
fn picker(ui: Ui) {
    let query = RwSignal::new(String::new());
    let family = RwSignal::new(String::new());
    let sub = RwSignal::new(String::new());
    let matches = Memo::new(move |_| {
        let query = query.get();
        ui.catalog.with(|c| {
            c.iter()
                .filter(|l| {
                    let hay = format!(
                        "{} {} {} {} {} {} {}",
                        l.id, l.name, l.family, l.subfamily, l.group, l.chemical_subfamily, l.tails
                    )
                    .to_lowercase();
                    query.split_whitespace().all(|term| hay.contains(term))
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let families = Memo::new(move |_| unique(matches.get().into_iter().map(|l| l.family)));
    let current = Memo::new(move |_| chosen(families.get(), family.get()));
    let subs = Memo::new(move |_| {
        unique(
            matches
                .get()
                .into_iter()
                .filter(|l| l.family == current.get())
                .map(|l| l.subfamily),
        )
    });
    let subcurrent = Memo::new(move |_| chosen(subs.get(), sub.get()));
    for (host, field, control, list, active, signal) in [
        (
            "lipid-families",
            "family",
            "lipid-subfamilies",
            families,
            current,
            family,
        ),
        (
            "lipid-subfamilies",
            "subfamily",
            "picker-lipids",
            subs,
            subcurrent,
            sub,
        ),
    ] {
        mount(host, move || {
            move || {
                list.get()
                    .into_iter()
                    .map(|name| {
                        let key = StoredValue::new(name.clone());
                        view! {
                            <button
                                class="secondary picker-choice"
                                data-family=(field == "family").then(|| name.clone())
                                data-subfamily=(field == "subfamily").then(|| name.clone())
                                aria-controls=control
                                aria-pressed=move || (active.get() == key.get_value()).to_string()
                                on:click=move |_| {
                                    signal.set(key.get_value());
                                    visible("preview-content", false);
                                }
                            >
                                {name.clone()}
                            </button>
                        }
                    })
                    .collect_view()
            }
        });
    }
    mount("picker-lipids", move || {
        move || {
            matches
                .get()
                .into_iter()
                .filter(|lipid| {
                    lipid.family == current.get() && lipid.subfamily == subcurrent.get()
                })
                .map(|lipid| picker_button(ui, lipid))
                .collect_view()
        }
    });
    click("add-lipid", move || {
        field("lipid-search").set_value("");
        query.set(String::new());
        visible("lipid-hover", false);
        let _ = dialog("lipid-picker").show_modal();
        focus("lipid-search");
    });
    click("close-picker", || dialog("lipid-picker").close());
    on("lipid-picker", "close", |_| {
        visible("preview-content", false)
    });
    on("preview-image", "error", |_| {
        visible("preview-image", false);
        visible("preview-error", true);
    });
    on("lipid-search", "input", move |_| {
        let q = field("lipid-search").value().trim().to_lowercase();
        let c = ui.catalog.get_untracked();
        if let Some(l) = c
            .iter()
            .find(|l| l.id.eq_ignore_ascii_case(&q))
            .or_else(|| c.iter().find(|l| l.subfamily.eq_ignore_ascii_case(&q)))
        {
            family.set(l.family.clone());
            sub.set(l.subfamily.clone());
        }
        query.set(q);
        visible("preview-content", false);
    });
    Effect::new(move |_| {
        visible("no-lipids", matches.get().is_empty());
        enabled("add-lipid", !ui.catalog.with(Vec::is_empty));
    });
}
fn picker_button(ui: Ui, l: Lipid) -> impl IntoView {
    let lipid = StoredValue::new(l.clone());
    let added = ui.plan.with(|p| {
        p.membrane
            .as_ref()
            .is_some_and(|m| m.rows.iter().any(|r| r.id == l.id))
    });
    view! {
        <button
            class="secondary catalog-lipid"
            data-lipid=l.id.clone()
            aria-label=format!(
                "{} {}{}",
                if added { "Already added" } else { "Add" },
                l.name,
                if l.tails.is_empty() { String::new() } else { format!(", {}", l.tails) },
            )
            aria-disabled=added.to_string()
            on:mouseenter=move |_| lipid.with_value(preview)
            on:focus=move |_| lipid.with_value(preview)
            on:mouseleave=move |_| visible("preview-content", false)
            on:blur=move |_| visible("preview-content", false)
            on:click=move |_| {
                if added {
                    return;
                }
                let id = lipid.get_value().id;
                update(
                    ui,
                    |m| {
                        m.rows
                            .push(Row {
                                id: id.clone(),
                                lower: 0.,
                                upper: 0.,
                            })
                    },
                );
                dialog("lipid-picker").close();
                request_animation_frame(move || focus(&format!("{id}-inner")));
            }
        >
            <span>
                {l.name.clone()}
                {(!l.tails.is_empty())
                    .then(|| view! { <span class="small lipid-tail">{l.tails.clone()}</span> })}
            </span>
            <span aria-hidden="true">{if added { "Added" } else { "+" }}</span>
        </button>
    }
}
fn unique(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = vec![];
    for s in items {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}
fn chosen(items: Vec<String>, s: String) -> String {
    if items.contains(&s) {
        s
    } else {
        items.into_iter().next().unwrap_or_default()
    }
}
fn row(ui: Ui, row: Row) -> impl IntoView {
    let l = ui
        .catalog
        .with_untracked(|c| c.iter().find(|l| l.id == row.id).cloned().unwrap());
    let lipid = StoredValue::new(l.clone());
    let key = StoredValue::new(row.id.clone());
    view! {
        <tr>
            <th scope="row">
                <button
                    id=format!("{}-name", l.id)
                    class="lipid-name"
                    aria-label=format!("{}: show chemical structure", l.name)
                    on:mouseenter=move |_| lipid.with_value(hover)
                    on:focus=move |_| lipid.with_value(hover)
                    on:click=move |_| lipid.with_value(hover)
                    on:mouseleave=move |_| visible("lipid-hover", false)
                    on:blur=move |_| visible("lipid-hover", false)
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
                    view! {
                        <td>
                            <div class="percentage-count">
                                <span class="percentage-field">
                                    <input
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
                                ></span>
                                <span
                                    class="lipid-adjustment"
                                    hidden
                                    id=format!("{id}-adjustment")
                                ></span>
                                <span class="lipid-count" id=format!("{id}-count")></span>
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
                        visible("lipid-hover", false);
                        focus("add-lipid");
                    }
                >
                    "×"
                </button>
            </td>
        </tr>
    }
}
fn svg(tag: &str, attrs: &[(&str, String)]) -> web_sys::Element {
    let e = document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
        .unwrap();
    for (k, v) in attrs {
        e.set_attribute(k, v).unwrap();
    }
    e
}
fn chart(side: &str, counts: &[(String, u32, usize)]) {
    let chart = el(&format!("{side}-pie"));
    let legend = el(&format!("{side}-legend"));
    chart.set_text_content(None);
    legend.set_text_content(None);
    let counts: Vec<_> = counts.iter().filter(|(_, count, _)| *count > 0).collect();
    let total: u32 = counts.iter().map(|(_, n, _)| n).sum();
    chart
        .set_attribute("data-total", &total.to_string())
        .unwrap();
    let description = if counts.is_empty() {
        "no computed counts".into()
    } else {
        counts
            .iter()
            .map(|(id, n, _)| format!("{id} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    chart
        .set_attribute(
            "aria-label",
            &format!(
                "{}: {description}",
                if side == "inner" { "Inner" } else { "Outer" }
            ),
        )
        .unwrap();
    let circle = |color: &str| {
        svg(
            "circle",
            &[
                ("cx", "100".into()),
                ("cy", "100".into()),
                ("r", "92".into()),
                ("fill", color.into()),
            ],
        )
    };
    if total == 0 {
        chart.append_child(&circle("#ecf0ed")).unwrap();
        let dash = svg(
            "text",
            &[
                ("x", "100".into()),
                ("y", "106".into()),
                ("text-anchor", "middle".into()),
                ("fill", "#52615c".into()),
            ],
        );
        dash.set_text_content(Some("—"));
        chart.append_child(&dash).unwrap();
        return;
    }
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let point = |a: f64| format!("{} {}", 100. + 92. * a.cos(), 100. + 92. * a.sin());
    for (id, n, i) in counts {
        let color = PALETTE[i % PALETTE.len()];
        let end = angle + std::f64::consts::TAU * f64::from(*n) / f64::from(total);
        let slice = if *n == total {
            circle(color)
        } else {
            svg(
                "path",
                &[
                    (
                        "d",
                        format!(
                            "M 100 100 L {} A 92 92 0 {} 1 {} Z",
                            point(angle),
                            u8::from(end - angle > std::f64::consts::PI),
                            point(end)
                        ),
                    ),
                    ("fill", color.into()),
                    ("stroke", "white".into()),
                    ("stroke-width", "1".into()),
                ],
            )
        };
        angle = end;
        slice.set_attribute("data-lipid", id).unwrap();
        slice.set_attribute("data-count", &n.to_string()).unwrap();
        let title = svg("title", &[]);
        title.set_text_content(Some(&format!("{id}: {n}")));
        slice.append_child(&title).unwrap();
        chart.append_child(&slice).unwrap();
        let li = document().create_element("li").unwrap();
        for (class, text) in [
            ("pie-key", String::new()),
            ("", id.clone()),
            ("pie-value", n.to_string()),
        ] {
            let span = document().create_element("span").unwrap();
            span.set_class_name(class);
            span.set_text_content(Some(&text));
            if class == "pie-key" {
                span.set_attribute("style", &format!("background:{color}"))
                    .unwrap();
            }
            li.append_child(&span).unwrap();
        }
        legend.append_child(&li).unwrap();
    }
}
fn percent(n: f64) -> String {
    format!("{}%", (n * 1000.).round() / 1000.)
}
fn refresh(ui: Ui, m: &Membrane, c: &[Lipid]) {
    let normalized = m.normalized(c).unwrap_or_default();
    let geometry = ui.result.with(|r| {
        r.as_ref()
            .filter(|_| ui.confirmed.get()[5])
            .map(|r| r.geometry.clone())
            .unwrap_or(Value::Null)
    });
    let estimates = m.estimates(c, &geometry).and_then(|v| {
        if !field("cell-size").check_validity() {
            Err("Enter a valid cell length, in steps of 0.1 nm.".into())
        } else if !field("xy-margin").check_validity() {
            Err("Enter a finite, nonnegative XY margin.".into())
        } else {
            if !v.iter().any(|(_, lo, _)| *lo > 0) || !v.iter().any(|(_, _, up)| *up > 0) {
                return Err("The cell is too small for a lipid in each leaflet.".into());
            }
            Ok(v)
        }
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
    let estimated = (ui.has_protein() || ui.has_ligands()) && geometry.is_null();
    let resolved = geometry["xy_diameter_nm"].as_f64().map(|d| {
        m.size_nm
            .max(((d + 2. * m.margin_nm) / (3f64.sqrt() / 2.) * 10.).ceil() / 10.)
            .max(geometry["minimum_cell_size_nm"].as_f64().unwrap_or(0.))
    });
    label(
        "resolved-cell",
        &resolved
            .map(|s| format!("Cell length after orientation: {s:.1} nm"))
            .unwrap_or_default(),
    );
    let total_area = [area[0] + protein_area[0], area[1] + protein_area[1]];
    visible("composition-estimate", estimated && !estimates.is_empty());
    notice(
        "composition-error",
        if m.rows.is_empty() { "" } else { &error },
    );
    let valid = !estimates.is_empty();
    let mismatch = if total_area[0].min(total_area[1]) > 0. {
        100. * (total_area[0] - total_area[1]).abs() / total_area[0].min(total_area[1])
    } else {
        0.
    };
    visible("balance-controls", valid && mismatch > 5.);
    options(
        "balancing-lipid",
        m.rows
            .iter()
            .filter(|r| r.lower + r.upper > 0.)
            .map(|r| (r.id.clone(), r.id.clone())),
    );
    enabled("balance", valid && m.balancing_lipid.is_none());
    for (i, side) in ["inner", "outer"].into_iter().enumerate() {
        let mut counts = vec![];
        let mut sum = 0.;
        for (j, r) in m.rows.iter().enumerate() {
            let id = format!("{}-{side}", r.id);
            if document()
                .get_element_by_id(&(id.clone() + "-count"))
                .is_none()
            {
                continue;
            }
            let value = normalized
                .iter()
                .find(|n| n.id == r.id)
                .map(|n| if i == 0 { n.lower } else { n.upper })
                .or((!normalized.is_empty()).then_some(0.));
            let pct = value.map(percent).unwrap_or_else(|| "—".into());
            label(&(id.clone() + "-normalized"), &pct);
            sum += value.unwrap_or_default();
            el(&(id.clone() + "-normalized"))
                .set_attribute(
                    "aria-label",
                    &format!("{} {side} normalized percentage: {pct}", r.id),
                )
                .unwrap();
            let count = estimates
                .iter()
                .find(|(n, _, _)| n.id == r.id)
                .map(|(_, l, u)| if i == 0 { *l } else { *u })
                .unwrap_or_default();
            counts.push((r.id.clone(), count, j));
            label(
                &(id.clone() + "-count"),
                &if valid {
                    format!("({count})")
                } else {
                    "(—)".into()
                },
            );
            let d = if m.balancing_lipid.as_ref() == Some(&r.id) {
                delta[i]
            } else {
                0
            };
            notice(
                &(id + "-adjustment"),
                &if d == 0 {
                    String::new()
                } else {
                    format!("{d:+}")
                },
            );
        }
        chart(side, &counts);
        label(
            &format!("{side}-total"),
            &if normalized.is_empty() {
                "—".into()
            } else {
                percent(sum)
            },
        );
        label(
            &format!("{side}-total-count"),
            &if valid {
                format!("({})", counts.iter().map(|(_, n, _)| n).sum::<u32>())
            } else {
                "(—)".into()
            },
        );
        let id = format!("{side}-area");
        label(
            &id,
            &if valid {
                format!("{:.3}", area[i])
            } else {
                "—".into()
            },
        );
        if total_area[i] > total_area[1 - i] {
            let excess = document().create_element("span").unwrap();
            excess.set_class_name("area-excess");
            excess.set_text_content(Some(&format!("(+{}%)", (100. * mismatch).round() / 100.)));
            el(&id).append_child(&excess).unwrap();
        }
    }
    notice(
        "combined-total",
        if m.per_leaflet {
            ""
        } else if normalized.is_empty() {
            "Normalized membrane total: —"
        } else {
            "Normalized membrane total: 100%"
        },
    );
    enabled("build", valid && ui.components_ready() && !ui.busy.get());
}
pub(super) fn setup(ui: Ui) {
    picker(ui);
    Effect::new(move |_| {
        let built = ui.confirmed.get()[5] && ui.plan.with(|p| p.membrane.is_some());
        visible("built-membrane", built);
        visible("composition-controls", !built);
    });
    listen(window().as_ref(), "scroll", |_| {
        visible("lipid-hover", false)
    });
    listen(window().as_ref(), "keydown", |e| {
        if e.dyn_ref::<web_sys::KeyboardEvent>()
            .is_some_and(|e| e.key() == "Escape")
        {
            visible("lipid-hover", false);
        }
    });
    mount("lipid-rows", move || {
        view! {
            <For
                each=move || {
                    ui.plan
                        .with(|p| p.membrane.as_ref().map(|m| m.rows.clone()).unwrap_or_default())
                }
                key=|r| r.id.clone()
                children=move |r| row(ui, r)
            />
        }
    });
    on("add-membrane", "change", move |_| {
        ui.plan.update(|p| {
            if field("add-membrane").checked() {
                p.membrane = Some(ui.saved_membrane.get_value());
            } else if let Some(m) = p.membrane.take() {
                ui.saved_membrane.set_value(m);
            }
        });
        ui.set_done(3, false);
        ui.set_done(4, false);
        ui.set_done(5, false);
    });
    for id in ["per-leaflet", "per-membrane"] {
        on(id, "change", move |_| {
            update(ui, |m| m.per_leaflet = field("per-leaflet").checked())
        });
    }
    on("cell-size", "input", move |_| {
        update(ui, |m| m.size_nm = field("cell-size").value_as_number())
    });
    on("xy-margin", "input", move |_| {
        update(ui, |m| m.margin_nm = field("xy-margin").value_as_number())
    });
    click("balance", move || {
        update(ui, |m| {
            m.balancing_lipid = Some(select("balancing-lipid").value())
        })
    });
    on("balancing-lipid", "change", move |_| update(ui, |_| {}));
    for id in ["build", "continue-no-membrane"] {
        click(id, move || {
            ui.set_done(3, true);
            ui.go(4);
        });
    }
    click("edit-membrane", move || {
        ui.set_done(3, false);
        ui.set_done(5, false);
        visible("composition-controls", true);
        visible("built-membrane", false);
    });
    Effect::new(move |_| {
        let m = ui.plan.get().membrane;
        visible("membrane-composition", m.is_some());
        visible("no-membrane-note", m.is_none());
        visible("no-membrane-actions", m.is_none());
        enabled(
            "continue-no-membrane",
            ui.components_ready() && (ui.has_protein() || ui.has_ligands()),
        );
        if ui.has_protein() || ui.has_ligands() {
            el("cell-size").remove_attribute("max").unwrap();
        } else {
            el("cell-size").set_attribute("max", "12").unwrap();
        }
        enabled("composition-controls", !ui.busy.get());
        enabled("add-membrane", !ui.busy.get());
        if let Some(m) = m {
            let c = ui.catalog.get();
            refresh(ui, &m, &c);
        }
    });
}
