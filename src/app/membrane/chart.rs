use super::*;
use crate::viewer::PALETTE;

#[component]
pub(super) fn LeafletChart(side: usize, composition: Memo<Composition>) -> impl IntoView {
    let name = ["inner", "outer"][side];
    let title = ["Inner", "Outer"][side];
    let counts = move || {
        composition.with(|c| {
            c.leaflets[side]
                .counts
                .iter()
                .filter(|(_, n, _)| *n > 0)
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let total = move || counts().iter().map(|(_, n, _)| *n).sum::<u32>();
    let label = move || {
        let rows = counts();
        let text = if rows.is_empty() {
            "no computed counts".into()
        } else {
            rows.into_iter()
                .map(|(id, n, _)| format!("{id} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!("{title}: {text}")
    };
    let slices = move || {
        let total = total();
        if total == 0 {
            return view! {
                <circle cx="100" cy="100" r="92" fill="#ecf0ed" />
                <text x="100" y="106" text-anchor="middle" fill="#52615c">
                    "—"
                </text>
            }
            .into_any();
        }
        let mut start = -std::f64::consts::FRAC_PI_2;
        counts()
            .into_iter()
            .map(|(id, n, j)| {
                let color = PALETTE[j % PALETTE.len()];
                let tip = format!("{id}: {n}");
                if n == total {
                    return view! {
                        <circle cx="100" cy="100" r="92" fill=color data-lipid=id data-count=n>
                            <title>{tip}</title>
                        </circle>
                    }
                    .into_any();
                }
                let end = start + std::f64::consts::TAU * f64::from(n) / f64::from(total);
                let point = |a: f64| format!("{} {}", 100. + 92. * a.cos(), 100. + 92. * a.sin());
                let path = format!(
                    "M 100 100 L {} A 92 92 0 {} 1 {} Z",
                    point(start),
                    u8::from(end - start > std::f64::consts::PI),
                    point(end)
                );
                start = end;
                view! {
                    <path
                        d=path
                        fill=color
                        stroke="white"
                        stroke-width="1"
                        data-lipid=id
                        data-count=n
                    >
                        <title>{tip}</title>
                    </path>
                }
                .into_any()
            })
            .collect_view()
            .into_any()
    };
    view! {
        <figure>
            <figcaption>{title}</figcaption>
            <svg
                id=format!("{name}-pie")
                viewBox="0 0 200 200"
                role="img"
                aria-label=label
                data-total=total
            >
                {slices}
            </svg>
            <ul id=format!("{name}-legend") class="pie-legend">
                {move || {
                    counts()
                        .into_iter()
                        .map(|(id, n, j)| {
                            view! {
                                <li>
                                    <span
                                        class="pie-key"
                                        style=format!("background:{}", PALETTE[j % PALETTE.len()])
                                    ></span>
                                    <span>{id}</span>
                                    <span class="pie-value">{n}</span>
                                </li>
                            }
                        })
                        .collect_view()
                }}
            </ul>
        </figure>
    }
}
