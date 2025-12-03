Here’s a clean, concise **markdown summary** of the shoreline/water rendering approach, written in a way that should plug nicely into a coding assistant:

---

# Water & Coast Rendering Approach

This document describes how to render shoreline water in a Graal-Kingdoms-style 2.5D terrain engine where **land tiles use height-warped quads**, but **water should remain flat** and use **color/alpha blending** to create a smooth beach→foam→shallow→deep gradient.

---

## 1. Tile Classification

From the heightmap (or world data), classify each tile:

* **LAND** — height ≥ `SEA_LEVEL`
* **WATER** — height < `SEA_LEVEL`
* **COAST** — a LAND tile with ≥1 WATER neighbor (4- or 8-way)
* **SHALLOW_WATER** — WATER tile with distance 1–2 from LAND
* **DEEP_WATER** — WATER tile with distance >2 from LAND

Optional: compute a distance field from LAND to get smoother depth values.

---

## 2. Mesh Rules

### Land Tiles

* Render using existing 2.5D mesh:

  * Each tile is a quad whose corners are Y-offset using the heightmap.
  * `(height_difference * vertical_scale)` produces the slope.

### Water Tiles

* **Water tiles do NOT use height offsets.**
* Render all water tiles as **flat quads** at constant Y:

  ```rust
  let water_y = SEA_LEVEL;
  v0.y = v1.y = v2.y = v3.y = water_y;
  ```

---

## 3. Water Depth Coloring

Assign colors or texture variants based on distance from land:

* **Shallow water** (distance 1–2): bright turquoise / light blue
* **Deep water** (distance >2): darker blue

Color can be computed by interpolation:

```rust
let depth = distance_to_land[x][y] as f32;
let t = (depth / MAX_DEPTH).clamp(0.0, 1.0);
let color = lerp(SHALLOW_COLOR, DEEP_COLOR, t);
```

Alternatively use separate textures for shallow/deep.

---

## 4. Shoreline / Foam Band

Render a soft shoreline transition on COAST tiles:

* Draw a **foam/shore mask texture** on coast tiles.
* The texture:

  * is white near the sand edge,
  * fades to transparent toward the water,
  * uses **alpha blending** to avoid square edges.

Placement:

1. Draw land (height-warped).
2. Draw shoreline mask (alpha-blended).
3. Draw water layer (flat, tinted by depth).

OR:

1. Land → 2. Water (semi-transparent) → 3. Foam overlay.
   (Choose order based on desired look.)

### Tile Masking

Either use:

* A **single generic foam texture** centered on coast tiles (simple), or
* A full **autotile mask set** (better):

  * Uses neighbor configuration (N/E/S/W/diagonals) to choose correct coast shape.

---

## 5. Rendering Order Summary

Final recommended draw order:

1. **Land mesh** (height-warped quads)
2. **Water quads** (flat plane, with shallow/deep coloring)
3. **Coastline foam masks** (alpha-blended quads on coastal tiles)

This reproduces the GK shoreline effect:
**sand → bright foam → shallow water → deep water**, without height distortion in the water.

---

If you want, I can also provide a **Rust-ready pseudocode version** of the tile classification + rendering pipeline.
