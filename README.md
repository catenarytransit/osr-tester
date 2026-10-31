# OSR Tester

Interactive [MOTIS OSR](https://github.com/motis-project/osr) routing debugger using the
native [Catenary osr-ffi](https://github.com/catenarytransit/osr-ffi) bridge,
an Axum Rust API, Svelte and MapLibre. The initial map is centered on UC Irvine.

## Requirements

Linux, Rust (edition 2024), a C++23 compiler, CMake, Git, Ninja/Make, Node 20+,
and the native requirements of `osr-ffi`. Native OSR compilation can take a while.
The bridge builds OSR automatically when building the Rust backend; use
`OSR_BUILD_JOBS=1` on memory-constrained machines.

**Important:** `osr-ffi` loads an **extracted OSR graph directory**.
It does not accept an unprocessed `.osm.pbf` at runtime.

## Import an OSM extract

Get an OSM PBF containing the area you wish to test (for UCI, an Orange County or
California extract). Install/build the upstream [MOTIS OSR](https://github.com/motis-project/osr)
CLI following its README, so that `osr-extract` is on your PATH:

```bash
# Replace the source filename with your downloaded extract.
mkdir -p data/irvine
osr-extract -i /path/to/orange-county.osm.pbf -o "$PWD/data/irvine"
```

The CLI preprocesses roads, routing graph and spatial lookup files. For journeys
across national/state boundaries, use **one merged PBF** extracted into a single
graph: independent extracted graphs do not interconnect automatically.

## Develop locally

Terminal 1:

```bash
cd backend
OSR_DATA_DIR="$(realpath ../data/irvine)" cargo run --release
```

The server opens the graph once at startup and listens on **127.0.0.1:3000**.
Change with `BIND_ADDR`. If the graph cannot be opened the server exits with
an error; it never returns fabricated routes. On Linux, the backend build copies `libcatenary_osr_bridge.so` into
`backend/target/release/lib/` (or `target/debug/lib/`) and sets the executable's
`$ORIGIN/lib` RUNPATH. When deploying, copy the executable **and** its `lib/`
directory together. Other system-native dependencies must also be installed.
Verify with `ldd target/release/osr-tester` and
`readelf -d target/release/osr-tester | grep RUNPATH`.

Terminal 2:

```bash
cd frontend
npm install
npm run dev
```

Open the URL printed by Vite (normally **http://127.0.0.1:5173**). The development
proxy forwards `/api` to the Rust backend.

## Production run

```bash
cd frontend
npm ci
npm run build
cd ../backend
OSR_DATA_DIR="$(realpath ../data/irvine)" STATIC_DIR="$(realpath ../frontend/dist)" cargo run --release
```

Open **http://127.0.0.1:3000** to use the built SPA and its API on the same
origin. If you have not generated an npm lockfile yet, use `npm install` in
place of `npm ci` for the initial installation.

## API

`GET /api/health`: process health.

`GET /api/info`: native graph node/way counts, elevation availability,
supported profiles and algorithms.

`POST /api/route`:

```bash
curl -s http://127.0.0.1:3000/api/route \
  -H 'Content-Type: application/json' \
  -d '{
    "from": {"lat": 33.6465, "lon": -117.8427},
    "to": {"lat": 33.6823, "lon": -117.8265},
    "profile": "foot",
    "algorithm": "bidirectional_a_star",
    "max_cost": 86400,
    "max_match_distance_meters": 100,
    "debug_snapping": true
  }'
```

Response contains `found`, travel `duration_seconds`, `distance_meters`,
`cost` (generalized routing weight, not always time), elevation statistics,
`geometry` as a GeoJSON `MultiLineString` in **[longitude, latitude]** order,
individual OSM path segments, and `diagnostics` with native algorithm
`routing_ms`, `queue_ms`, `snapping_ms`, `pre_response_ms`, graph
counts, and up to five snap candidates per endpoint. The frontend measures
additional browser round-trip latency independently.

Algorithm choices: `dijkstra`, `bidirectional_a_star`. The FFI currently
rejects direct `AStar` (numeric 2) in its native adapter, so the UI does
not expose it. The UI omits sharing profiles because mobility overlays
are not constructed by this bridge. A missing route returns `found:false`
and no geometry; it never uses a straight-line substitute.

The C++ router is **not Sync**. One bounded (64-entry) dedicated worker owns
the native router and processes requests sequentially. Requests beyond its
queue return HTTP 429. Route timings separate native computation from queue
wait and optional candidate lookup. This tester is designed for benchmarking,
not exposing to untrusted public traffic: bind locally or place behind
authentication and request limits.

## Checks

```bash
cd backend && cargo fmt --check && cargo test
cd ../frontend && npm run check && npm run build
```

The Rust tests cover input bounds, profile mappings and GeoJSON coordinate order.
For end-to-end route correctness and performance, use an extracted test PBF
with known street connections, one-way restrictions and unreachable pairs.
