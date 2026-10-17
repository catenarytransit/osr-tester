<script lang="ts">
  import { onMount } from 'svelte';
  import maplibregl, { type Map, type Marker } from 'maplibre-gl';
  import 'maplibre-gl/dist/maplibre-gl.css';

  type Point = { lat: number; lon: number };
  type RouteResponse = {
    found: boolean;
    duration_seconds: number | null;
    distance_meters: number | null;
    cost: number | null;
    elevation_up: number | null;
    elevation_down: number | null;
    uses_elevator: boolean | null;
    geometry: { type: 'MultiLineString'; coordinates: number[][][] } | null;
    segments: Array<{ osm_way_id: string | null; distance_meters: number; duration_seconds: number; cost: number }>;
    diagnostics: {
      queue_ms: number; routing_ms: number; snapping_ms: number; pre_response_ms: number;
      segment_count: number; geometry_point_count: number;
      graph: { node_count: number; way_count: number; has_elevations: boolean };
      from_candidates: Array<{osm_way_id: string; distance_to_way_meters: number}>;
      to_candidates: Array<{osm_way_id: string; distance_to_way_meters: number}>;
      snapping_error: string | null;
    };
  };

  const profiles = [
    ['foot', 'Walking'], ['wheelchair', 'Wheelchair'], ['bike', 'Bicycle'],
    ['bike_fast', 'Fast bicycle'], ['bike_elevation_low', 'Bike (avoid climbs)'],
    ['bike_elevation_high', 'Bike (prefer climbs)'], ['car', 'Car'], ['hgv', 'Heavy goods'],
    ['car_drop_off', 'Car drop-off'], ['car_drop_off_wheelchair', 'Accessible drop-off'],
    ['car_parking', 'Car parking'], ['car_parking_wheelchair', 'Accessible parking'],
    ['bus', 'Bus'], ['railway', 'Railway'], ['ferry', 'Ferry']
  ];
  let mapElement: HTMLDivElement;
  let map: Map;
  let startMarker: Marker;
  let endMarker: Marker;
  let from = $state<Point>({ lat: 33.6465, lon: -117.8427 });
  let to = $state<Point>({ lat: 33.6823, lon: -117.8265 });
  let profile = $state('foot');
  let algorithm = $state('bidirectional_a_star');
  let maxCost = $state(86400);
  let snapRadius = $state(100);
  let debugSnapping = $state(true);
  let loading = $state(false);
  let error = $state('');
  let response = $state<RouteResponse | null>(null);
  let browserMs = $state<number | null>(null);
  let apiGraph = $state<{node_count:number;way_count:number;has_elevations:boolean} | null>(null);
  let sequence = 0;
  let controller: AbortController | null = null;
  let debounce: ReturnType<typeof setTimeout>;

  const fmt = (n: number | null | undefined, digits = 1) =>
    n == null ? '—' : n.toLocaleString(undefined, { maximumFractionDigits: digits });

  const routeFeature = (data: RouteResponse | null) => ({
    type: 'FeatureCollection',
    features: data?.geometry ? [{
      type: 'Feature', properties: {},
      geometry: data.geometry
    }] : []
  });

  function updateLine() {
    if (!map || !map.isStyleLoaded()) return;
    const source = map.getSource('route') as maplibregl.GeoJSONSource | undefined;
    if (source) source.setData(routeFeature(response) as GeoJSON.FeatureCollection);
  }

  function updateMarkerPositions() {
    startMarker?.setLngLat([from.lon, from.lat]);
    endMarker?.setLngLat([to.lon, to.lat]);
  }

  function setPoint(which: 'from' | 'to', point: Point) {
    if (which === 'from') from = point; else to = point;
    updateMarkerPositions();
    scheduleRoute();
  }

  function scheduleRoute() {
    clearTimeout(debounce);
    debounce = setTimeout(() => void requestRoute(), 160);
  }

  async function requestRoute() {
    const id = ++sequence;
    controller?.abort();
    controller = new AbortController();
    loading = true;
    error = '';
    const start = performance.now();
    try {
      const res = await fetch('/api/route', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        signal: controller.signal,
        body: JSON.stringify({
          from, to, profile, algorithm,
          max_cost: maxCost,
          max_match_distance_meters: snapRadius,
          debug_snapping: debugSnapping
        })
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error ?? `HTTP ${res.status}`);
      if (id !== sequence) return;
      response = data as RouteResponse;
      browserMs = performance.now() - start;
      updateLine();
    } catch (e) {
      if (id !== sequence || (e instanceof Error && e.name === 'AbortError')) return;
      error = e instanceof Error ? e.message : String(e);
      response = null;
      updateLine();
    } finally {
      if (id === sequence) loading = false;
    }
  }

  function featureLine(map: Map) {
    if (map.getSource('route')) return;
    map.addSource('route', { type: 'geojson', data: routeFeature(response) as GeoJSON.FeatureCollection });
    map.addLayer({
      id: 'route-outline', type: 'line', source: 'route',
      paint: { 'line-color': '#ffffff', 'line-width': 8, 'line-opacity': 0.9 },
      layout: { 'line-join': 'round', 'line-cap': 'round' }
    });
    map.addLayer({
      id: 'route-line', type: 'line', source: 'route',
      paint: { 'line-color': '#1571e8', 'line-width': 5 },
      layout: { 'line-join': 'round', 'line-cap': 'round' }
    });
  }

  onMount(() => {
    map = new maplibregl.Map({
      container: mapElement,
      style: 'https://maps.catenarymaps.org/light-style.json',
      center: [-117.8443, 33.6488],
      zoom: 12.6
    });
    map.addControl(new maplibregl.NavigationControl(), 'top-right');
    map.on('style.load', () => featureLine(map));

    const makeMarker = (which: 'from' | 'to', point: Point, className: string) => {
      const el = document.createElement('div');
      el.className = 'route-pin ' + className;
      el.title = which === 'from' ? 'Drag route origin' : 'Drag route destination';
      const marker = new maplibregl.Marker({ element: el, draggable: true })
        .setLngLat([point.lon, point.lat]).addTo(map);
      marker.on('dragend', () => {
        const pos = marker.getLngLat();
        setPoint(which, {lat: pos.lat, lon: pos.lng});
      });
      return marker;
    };
    startMarker = makeMarker('from', from, 'origin');
    endMarker = makeMarker('to', to, 'destination');
    fetch('/api/info').then(r => r.json()).then(d => { apiGraph = d.graph; }).catch(() => {});
    void requestRoute();
    return () => {
      ++sequence;
      clearTimeout(debounce);
      controller?.abort();
      map.remove();
    };
  });
</script>

<svelte:head>
  <title>OSR Routing Debugger · Catenary</title>
  <meta name="description" content="Interactive MOTIS Open Street Router path and performance debugger" />
</svelte:head>

<div class="app">
  <aside class="sidebar">
    <header><div class="eyebrow">CATENARY · ROUTING LAB</div><h1>OSR Tester</h1><p>Native MOTIS routing via osr-ffi</p></header>
    <section>
      <h2>Route setup</h2>
      <p class="hint">Drag the green origin and red destination pins on the map.</p>
      <label for="profile">Travel mode</label>
      <select id="profile" bind:value={profile} onchange={scheduleRoute}>
        {#each profiles as p}<option value={p[0]}>{p[1]}</option>{/each}
      </select>
      <label for="algorithm">Search algorithm</label>
      <select id="algorithm" bind:value={algorithm} onchange={scheduleRoute}>
        <option value="bidirectional_a_star">Bidirectional A*</option>
        <option value="dijkstra">Dijkstra</option>
      </select>
      <div class="locations">
        <div><span class="point-dot green"></span><strong>Origin</strong><small>{from.lat.toFixed(6)}, {from.lon.toFixed(6)}</small></div>
        <div><span class="point-dot red"></span><strong>Destination</strong><small>{to.lat.toFixed(6)}, {to.lon.toFixed(6)}</small></div>
      </div>
      <div class="split">
        <div><label for="cost">Max cost</label><input id="cost" type="number" min="1" max="1000000" bind:value={maxCost} onchange={scheduleRoute} /></div>
        <div><label for="radius">Snap radius (m)</label><input id="radius" type="number" min="1" max="2000" bind:value={snapRadius} onchange={scheduleRoute} /></div>
      </div>
      <label class="checkbox"><input type="checkbox" bind:checked={debugSnapping} onchange={scheduleRoute}/> Collect snap candidates</label>
      <button onclick={requestRoute} disabled={loading}>{loading ? 'Routing…' : 'Run routing'}</button>
    </section>

    <section aria-live="polite">
      <h2>Result</h2>
      {#if error}<div class="error">{error}</div>{/if}
      {#if response}
        {#if response.found}
          <div class="bigstat">{fmt(response.duration_seconds, 0)} <span>seconds travel time</span></div>
          <div class="metrics">
            <div><small>Distance</small><strong>{fmt(response.distance_meters, 0)} m</strong></div>
            <div><small>Generalized cost</small><strong>{fmt(response.cost, 0)}</strong></div>
            <div><small>Uphill / downhill</small><strong>{fmt(response.elevation_up, 0)} / {fmt(response.elevation_down, 0)} m</strong></div>
            <div><small>Elevator</small><strong>{response.uses_elevator ? 'Yes' : 'No'}</strong></div>
          </div>
        {:else}
          <div class="error">No route found in the loaded graph. Check coverage, snapping radius and maximum cost.</div>
        {/if}
        <h3>Performance</h3>
        <div class="metrics">
          <div><small>Native route</small><strong>{fmt(response.diagnostics.routing_ms, 2)} ms</strong></div>
          <div><small>Worker queue</small><strong>{fmt(response.diagnostics.queue_ms, 2)} ms</strong></div>
          <div><small>Snap diagnostics</small><strong>{fmt(response.diagnostics.snapping_ms, 2)} ms</strong></div>
          <div><small>API pre-response</small><strong>{fmt(response.diagnostics.pre_response_ms, 2)} ms</strong></div>
          <div><small>Browser round trip</small><strong>{fmt(browserMs, 2)} ms</strong></div>
          <div><small>Segments / points</small><strong>{response.diagnostics.segment_count} / {response.diagnostics.geometry_point_count}</strong></div>
        </div>
        <details><summary>Snapping candidates</summary>
          {#if response.diagnostics.snapping_error}<p class="error">{response.diagnostics.snapping_error}</p>{/if}
          <h4>Origin</h4>
          {#each response.diagnostics.from_candidates as c}
            <div class="candidate">Way {c.osm_way_id}: {fmt(c.distance_to_way_meters, 2)} m</div>
          {/each}
          <h4>Destination</h4>
          {#each response.diagnostics.to_candidates as c}
            <div class="candidate">Way {c.osm_way_id}: {fmt(c.distance_to_way_meters, 2)} m</div>
          {/each}
        </details>
        <details><summary>Raw route JSON</summary><pre>{JSON.stringify(response, null, 2)}</pre></details>
      {/if}
    </section>
    {#if apiGraph}<footer>{fmt(apiGraph.node_count, 0)} nodes · {fmt(apiGraph.way_count, 0)} ways{apiGraph.has_elevations ? ' · elevation data' : ''}</footer>{/if}
  </aside>
  <main class="map" bind:this={mapElement}></main>
</div>
