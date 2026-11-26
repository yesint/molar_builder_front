// Round by largest remainder so the displayed percentages sum to exactly 100.
function distribute(values) {
  const maximum = Math.max(...values);
  const scaled = values.map(value => value / maximum);
  const total = scaled.reduce((sum, value) => sum + value, 0);
  const exact = scaled.map(value => value / total * 100000);
  const units = exact.map(Math.floor);
  const order = exact.map((value, index) => ({ index, remainder: value - units[index] }))
    .sort((a, b) => b.remainder - a.remainder);
  const remaining = 100000 - units.reduce((sum, value) => sum + value, 0);
  for (let i = 0; i < remaining; i++) units[order[i].index]++;
  return units.map(value => value / 1000);
}

export function normalizeComposition(rows, perLeaflet) {
  if (!rows.length) throw new Error("Add at least one lipid species.");
  for (const row of rows) {
    for (const side of ["upper", "lower"]) {
      if (typeof row[side] !== "number" || !Number.isFinite(row[side]) || row[side] < 0) {
        throw new Error(`Enter a finite, nonnegative percentage for ${row.id} in the ${side === "lower" ? "inner" : "outer"} leaflet.`);
      }
    }
  }
  for (const side of ["upper", "lower"]) {
    if (!rows.some(row => row[side] > 0)) throw new Error(`Add a positive percentage to the ${side === "lower" ? "inner" : "outer"} leaflet.`);
  }
  const result = rows.map(row => ({ ...row }));
  if (perLeaflet) {
    for (const side of ["upper", "lower"]) {
      const values = distribute(rows.map(row => row[side]));
      result.forEach((row, index) => { row[side] = values[index]; });
    }
  } else {
    const values = distribute(rows.flatMap(row => [row.upper, row.lower]));
    result.forEach((row, index) => { row.upper = values[2 * index]; row.lower = values[2 * index + 1]; });
  }
  for (const side of ["upper", "lower"]) {
    if (!result.some(row => row[side] > 0)) {
      throw new Error(`The ${side === "lower" ? "inner" : "outer"} leaflet rounds to zero. Increase its percentage before continuing.`);
    }
  }
  return result;
}

// Match areas::counts_for_cell and largest_remainder in bilayer_builder.
// Round intermediate values to f32, as the builder does for geometry and areas.
export function lipidCounts(rows, catalogue, size, perLeaflet, proteinAreas = { upper: 0, lower: 0 }) {
  validateProteinAreas(proteinAreas);
  if (!Number.isFinite(size) || size <= 0) throw new Error("Enter a positive cell length.");
  const normalized = normalizeComposition(rows, perLeaflet);
  const f = Math.fround;
  const sum = values => values.reduce((total, value) => f(total + value), 0);
  const areas = normalized.map(row => {
    const area = catalogue.find(lipid => lipid.id === row.id)?.area_nm2;
    if (!Number.isFinite(area) || area <= 0) throw new Error(`No lipid area is available for ${row.id}.`);
    return f(area);
  });
  const upper = normalized.map(row => f(row.upper));
  const lower = normalized.map(row => f(row.lower));
  const length = f(size);
  const radians = f(f(120) * f(Math.PI / 180));
  const cellArea = f(length * f(length * f(Math.sin(radians))));
  const free = { upper: f(cellArea - f(proteinAreas.upper)), lower: f(cellArea - f(proteinAreas.lower)) };
  if (free.upper <= 0 || free.lower <= 0) throw new Error("The protein occupies the entire cell area in a leaflet.");
  function allocate(weights, freeArea) {
    const total = sum(weights);
    const mean = f(sum(weights.map((value, i) => f(value * areas[i % areas.length]))) / total);
    const count = Math.round(f(freeArea / mean));
    const exact = weights.map(value => f(f(value / total) * count));
    const counts = exact.map(Math.floor);
    const order = exact.map((value, index) => ({ index, remainder: f(value - counts[index]) }))
      .sort((a, b) => b.remainder - a.remainder || a.index - b.index);
    const missing = count - counts.reduce((a, b) => a + b, 0);
    for (const { index } of order.slice(0, Math.max(0, missing))) counts[index]++;
    return counts;
  }
  let up, low;
  if (perLeaflet) {
    up = allocate(upper, free.upper);
    low = allocate(lower, free.lower);
  } else {
    // The builder resolves rounding ties in upper-species, lower-species order.
    const all = allocate([...upper, ...lower], f(free.upper + free.lower));
    up = all.slice(0, rows.length);
    low = all.slice(rows.length);
  }
  return rows.map((row, i) => ({ id: row.id, upper: up[i], lower: low[i] }));
}

function validateProteinAreas(areas) {
  for (const side of ["upper", "lower"]) {
    if (!Number.isFinite(areas[side]) || areas[side] < 0) throw new Error("Protein areas must be finite, nonnegative numbers.");
  }
}

export function monolayerAreas(counts, catalogue, proteinAreas = { upper: 0, lower: 0 }) {
  validateProteinAreas(proteinAreas);
  const areas = { ...proteinAreas };
  for (const row of counts) {
    const area = catalogue.find(lipid => lipid.id === row.id)?.area_nm2;
    if (!Number.isFinite(area) || area <= 0) throw new Error(`No lipid area is available for ${row.id}.`);
    for (const side of ["upper", "lower"]) areas[side] += row[side] * area;
  }
  return areas;
}

// How much larger one monolayer is, relative to the smaller monolayer.
export function areaMismatch(areas) {
  const smaller = Math.min(areas.upper, areas.lower);
  const difference = Math.abs(areas.upper - areas.lower);
  return smaller > 0 ? 100 * difference / smaller : difference > 0 ? Infinity : 0;
}

export function balanceLipid(counts, catalogue, id, proteinAreas = { upper: 0, lower: 0 }) {
  const areas = monolayerAreas(counts, catalogue, proteinAreas);
  const row = counts.find(item => item.id === id);
  const area = catalogue.find(item => item.id === id)?.area_nm2;
  if (!row || !Number.isFinite(area) || area <= 0) throw new Error("Select a balancing lipid.");
  const larger = areas.upper > areas.lower ? "upper" : "lower";
  const smaller = larger === "upper" ? "lower" : "upper";
  const gap = Math.abs(areas.upper - areas.lower);
  if (gap < 1e-9) return null;
  const ideal = gap / area;
  const totalLarger = counts.reduce((sum, item) => sum + item[larger], 0);
  const removable = Math.min(row[larger], totalLarger - 1);
  const candidates = [];
  for (const number of new Set([Math.floor(ideal), Math.ceil(ideal), removable])) {
    if (number <= 0) continue;
    for (const side of [larger, smaller]) {
      const delta = side === larger ? -number : number;
      if (delta < 0 && number > removable) continue;
      const residual = Math.abs(gap - number * area);
      if (residual >= gap - 1e-9) continue;
      candidates.push({ id, side, delta, residual });
    }
  }
  candidates.sort((a, b) => Math.abs(a.residual - b.residual) > 1e-9
    ? a.residual - b.residual : Math.abs(a.delta) - Math.abs(b.delta) || a.delta - b.delta);
  if (!candidates.length) return null;
  const { side, delta } = candidates[0];
  return { id, side, delta };
}

// Match the server's margin rule for the oriented solute in a hexagonal cell.
export function membraneCellSize(requested, margin, geometry) {
  if (!geometry) return requested;
  if (!Number.isFinite(geometry.xy_diameter_nm)) throw new Error("The prepared system has no XY extent. Restart the updated server.");
  const required = Math.ceil((geometry.xy_diameter_nm + 2 * margin) / (Math.sqrt(3) / 2) * 10) / 10;
  return Math.max(requested, required, geometry.minimum_cell_size_nm);
}
