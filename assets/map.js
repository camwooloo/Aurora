/* =====================================================================
   AuroraCast — map controller (Leaflet + wind particles + API bridge)
   Exposes window.AuroraMap. Called from Rust via dioxus::document::eval.
   ===================================================================== */
(function () {
  'use strict';

  const state = {
    map: null,
    velocityLayer: null,
    rainLayer: null,
    satLayer: null,
    tempLayer: null,
    cloudLayer: null,
    overlayLayer: null,
    rainFrames: [],
    satFrames: [],
    markers: [],
    currentLayer: 'wind',
    pendingInit: null,
  };

  const WIND_JSON_URL = 'https://onaci.github.io/leaflet-velocity/wind-global.json';

  // -------- helpers ----------------------------------------------------
  function waitForLeaflet() {
    return new Promise((resolve) => {
      if (window.L && window.L.velocityLayer) return resolve();
      const iv = setInterval(() => {
        if (window.L && window.L.velocityLayer) {
          clearInterval(iv);
          resolve();
        }
      }, 50);
    });
  }

  function emit(name, detail) {
    window.dispatchEvent(new CustomEvent('aurora:' + name, { detail }));
  }

  // -------- initialize -------------------------------------------------
  function debug(msg) {
    console.log('[AuroraMap] ' + msg);
    const el = document.getElementById('aurora-debug');
    if (el) el.textContent = msg;
  }

  function showDebugBanner(msg) {
    let el = document.getElementById('aurora-debug');
    if (!el) {
      el = document.createElement('div');
      el.id = 'aurora-debug';
      el.style.cssText = 'position:fixed;bottom:120px;left:50%;transform:translateX(-50%);background:#131822;color:#e6eaf2;padding:6px 12px;border:1px solid #252c3a;border-radius:8px;font:11px/1.4 monospace;z-index:9999;pointer-events:none;opacity:.9;max-width:90vw';
      document.body.appendChild(el);
    }
    el.textContent = msg;
  }

  async function init(containerId) {
    showDebugBanner('init: waiting for Leaflet…');
    await waitForLeaflet();
    showDebugBanner('init: Leaflet loaded');

    const el = document.getElementById(containerId);
    if (!el) {
      showDebugBanner('init: #' + containerId + ' not found');
      return;
    }
    const rect = el.getBoundingClientRect();
    showDebugBanner('init: #map found, size ' + Math.round(rect.width) + '×' + Math.round(rect.height));

    if (state.map) { state.map.remove(); state.map = null; }

    try {
      state.map = L.map(el, {
        center: [46.5, 8], zoom: 4, minZoom: 2, maxZoom: 12,
        worldCopyJump: true, zoomControl: false, preferCanvas: true,
        attributionControl: true, tap: true,
      });
    } catch (e) {
      showDebugBanner('init error: ' + e.message);
      return;
    }
    showDebugBanner('init: Leaflet map created');
    L.control.attribution({
      prefix: 'AuroraCast · Data: Open-Meteo, RainViewer, OpenStreetMap',
    }).addTo(state.map);

    // Base layers (dark map, then a dedicated top-level pane for labels so
    // they sit ABOVE weather overlays — otherwise cloud imagery washes them
    // out and countries become hard to read.)
    L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_nolabels/{z}/{x}/{y}{r}.png', {
      subdomains: 'abcd', maxZoom: 19, opacity: 0.95,
    }).addTo(state.map);

    if (!state.map.getPane('aurora-labels')) {
      const pane = state.map.createPane('aurora-labels');
      pane.style.zIndex = 650;   // above overlays (360) + markers (600)
      pane.style.pointerEvents = 'none';
      // Subtle drop-shadow makes labels pop on any overlay background.
      pane.style.filter = 'drop-shadow(0 0 1px rgba(0,0,0,.9))';
    }
    L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_only_labels/{z}/{x}/{y}{r}.png', {
      subdomains: 'abcd', maxZoom: 19, pane: 'aurora-labels',
    }).addTo(state.map);

    // Initialize wind particles
    try {
      const r = await fetch(WIND_JSON_URL);
      const data = await r.json();
      state.velocityLayer = L.velocityLayer({
        displayValues: false,
        data,
        maxVelocity: 25,
        velocityScale: 0.012,
        particleAge: 80,
        lineWidth: 1.4,
        particleMultiplier: 0.004,
        colorScale: ['#1a3a6e', '#4aa3ff', '#5cd389', '#ffd166', '#ff9f1c', '#ff6b6b', '#9b4dca'],
      });
      if (state.currentLayer === 'wind') state.velocityLayer.addTo(state.map);
    } catch (e) { console.warn('wind init failed', e); }

    // RainViewer frames
    try {
      const r = await fetch('https://api.rainviewer.com/public/weather-maps.json');
      const j = await r.json();
      state.rainFrames = [...(j.radar?.past || []), ...(j.radar?.nowcast || [])]
        .map(f => j.host + f.path + '/256/{z}/{x}/{y}/2/1_1.png');
      state.satFrames = (j.satellite?.infrared || [])
        .map(f => j.host + f.path + '/256/{z}/{x}/{y}/0/0_0.png');
    } catch (_) {}

    state.map.on('click', (e) => {
      emit('mapclick', { lat: e.latlng.lat, lon: e.latlng.lng });
    });

    // Handle container resize (mobile rotation, panels toggling)
    window.addEventListener('resize', () => {
      if (state.map) state.map.invalidateSize();
    });

    // Force a resize after mount so tiles populate
    setTimeout(() => state.map && state.map.invalidateSize(), 100);

    emit('ready', {});
    showDebugBanner('map ready — tap to dismiss');
    const banner = document.getElementById('aurora-debug');
    if (banner) setTimeout(() => banner.remove(), 5000);
  }

  // -------- layer switching -------------------------------------------
  function setLayer(name) {
    state.currentLayer = name;
    if (!state.map) return;
    // remove everything
    [state.velocityLayer, state.rainLayer, state.satLayer,
     state.tempLayer, state.cloudLayer, state.overlayLayer].forEach(l => {
      if (l && state.map.hasLayer(l)) state.map.removeLayer(l);
    });
    state.rainLayer = state.satLayer = state.tempLayer = state.cloudLayer = state.overlayLayer = null;

    switch (name) {
      case 'wind':
        if (state.velocityLayer) state.velocityLayer.addTo(state.map);
        break;
      case 'rain':
        setRainFrame(Math.floor(state.rainFrames.length / 2));
        break;
      case 'satellite':
        setSatFrame(state.satFrames.length - 1);
        break;
      case 'clouds':
        // Windy-style clouds: real forecast cloud cover %, rendered as soft
        // white patches with alpha = cloud%. Basemap shows through. All 240
        // hours are fetched once, so timeline scrub is instant (no network).
        loadAndRenderClouds();
        break;
      case 'snow':
        state.overlayLayer = buildGibsLayer(
          'MODIS_Terra_NDSI_Snow_Cover',
          8, 'png', { opacity: 0.55 }
        ).addTo(state.map);
        break;
      case 'temp':
      case 'pressure':
      case 'waves':
        // Sample Open-Meteo over a grid; paint a soft heatmap overlay.
        buildGridDataOverlay(name).then(layer => {
          if (layer && state.currentLayer === name && state.map) {
            if (state.overlayLayer && state.map.hasLayer(state.overlayLayer)) {
              state.map.removeLayer(state.overlayLayer);
            }
            state.overlayLayer = layer.addTo(state.map);
          }
        });
        scheduleGridRefresh(name);
        break;
    }
  }

  function setRainFrame(i) {
    if (state.rainLayer) state.map.removeLayer(state.rainLayer);
    if (!state.rainFrames.length) return;
    const idx = Math.max(0, Math.min(state.rainFrames.length - 1, i));
    state.rainLayer = L.tileLayer(state.rainFrames[idx], { opacity: 0.75, maxZoom: 12 }).addTo(state.map);
  }
  function setSatFrame(i) {
    if (state.satLayer) state.map.removeLayer(state.satLayer);
    if (!state.satFrames.length) return;
    const idx = Math.max(0, Math.min(state.satFrames.length - 1, i));
    state.satLayer = L.tileLayer(state.satFrames[idx], { opacity: 0.7, maxZoom: 12 }).addTo(state.map);
  }
  // Clouds use the same IR satellite source but in its own layer slot with
  // a lower opacity so the basemap stays readable underneath.
  function setCloudFrame(i) {
    if (state.cloudLayer) state.map.removeLayer(state.cloudLayer);
    if (!state.satFrames.length) return;
    const idx = Math.max(0, Math.min(state.satFrames.length - 1, i));
    state.cloudLayer = L.tileLayer(state.satFrames[idx], {
      opacity: 0.55,  // lighter than the Satellite layer so map shows through
      maxZoom: 12,
    }).addTo(state.map);
  }

  function setTime(hourOffset, totalHours) {
    if (!state.map) return;
    const frac = hourOffset / totalHours;
    if (state.currentLayer === 'rain' && state.rainFrames.length) {
      setRainFrame(Math.floor(frac * state.rainFrames.length));
    }
    if (state.currentLayer === 'satellite' && state.satFrames.length) {
      setSatFrame(Math.floor(frac * state.satFrames.length));
    }
    if (state.currentLayer === 'clouds') {
      if (cloudGrid) {
        // Pure canvas re-paint from cached forecast → smooth dragging
        const hours = (cloudGrid.hourly[0] || []).length;
        const h = Math.min(hours - 1, Math.max(0, hourOffset));
        renderCloudFrame(h);
      } else {
        loadAndRenderClouds();  // first call triggers the fetch
      }
    }
    if (state.currentLayer === 'snow') {
      // Step snow overlay one MODIS frame per day of timeline offset.
      const d = gibsDateFromHourOffset(hourOffset);
      if (state.overlayLayer && state.map.hasLayer(state.overlayLayer)) {
        state.map.removeLayer(state.overlayLayer);
      }
      state.overlayLayer = buildGibsLayer(
        'MODIS_Terra_NDSI_Snow_Cover', 8, 'png', { opacity: 0.55, date: d }
      ).addTo(state.map);
    }
    if (state.currentLayer === 'wind' && state.velocityLayer) {
      try { state.velocityLayer.setOptions({ particleAge: 60 + ((hourOffset * 7) % 50) }); }
      catch (_) {}
    }
  }

  function panTo(lat, lon, zoom) {
    if (!state.map) return;
    state.map.flyTo([lat, lon], zoom || Math.max(state.map.getZoom(), 7), { duration: 0.8 });
  }

  function showPopup(lat, lon, html) {
    if (!state.map) return;
    L.popup({ className: 'ac-popup', maxWidth: 260 })
      .setLatLng([lat, lon])
      .setContent(html)
      .openOn(state.map);
  }

  function invalidate() {
    if (state.map) state.map.invalidateSize();
  }

  // -------- Windy-style cloud overlay (Open-Meteo forecast grid) ------
  // Fetches hourly cloud cover % for a grid of points covering the map,
  // then paints soft white circles (alpha = cloud%) onto a canvas. Scrubbing
  // the timeline is instant because all 240 hours are cached locally.
  let cloudGrid = null;           // {lats, lons, hourly[point][hour], bounds}
  let cloudHourIdx = 0;
  let cloudLoading = false;
  let cloudAbort = null;

  async function loadAndRenderClouds() {
    if (!state.map) return;
    // Reuse cache if viewport still roughly covered
    const vb = state.map.getBounds();
    if (cloudGrid && cloudGrid.bounds.contains(vb.getCenter())) {
      renderCloudFrame(cloudHourIdx);
      return;
    }
    if (cloudLoading) return;
    cloudLoading = true;
    if (cloudAbort) cloudAbort.abort();
    cloudAbort = new AbortController();

    // Build a grid covering ~2x the visible viewport so pans feel continuous.
    const pad = 0.8;
    const cLat = (vb.getNorth() + vb.getSouth()) / 2;
    const cLon = (vb.getWest() + vb.getEast()) / 2;
    const halfH = (vb.getNorth() - vb.getSouth()) / 2 * (1 + pad);
    const halfW = (vb.getEast() - vb.getWest()) / 2 * (1 + pad);

    const ROWS = 9, COLS = 12;      // 108 points — safely under URL limit
    const lats = [], lons = [];
    for (let r = 0; r < ROWS; r++) {
      for (let c = 0; c < COLS; c++) {
        const lat = Math.max(-85, Math.min(85,
          cLat + halfH - (r + 0.5) * (2 * halfH) / ROWS));
        let lon = cLon - halfW + (c + 0.5) * (2 * halfW) / COLS;
        lon = ((lon + 540) % 360) - 180;
        lats.push(lat.toFixed(3));
        lons.push(lon.toFixed(3));
      }
    }
    const url = `https://api.open-meteo.com/v1/forecast?latitude=${lats.join(',')}&longitude=${lons.join(',')}&hourly=cloud_cover&forecast_days=10&timezone=UTC`;

    showDebugBanner('Clouds: fetching forecast grid…');
    let arr;
    try {
      const r = await fetch(url, { signal: cloudAbort.signal });
      const j = await r.json();
      arr = Array.isArray(j) ? j : [j];
    } catch (e) {
      cloudLoading = false;
      if (e.name !== 'AbortError') showDebugBanner('Clouds fetch failed: ' + e.message);
      return;
    }

    cloudGrid = {
      lats: lats.map(Number),
      lons: lons.map(Number),
      hourly: arr.map(p => (p && p.hourly && p.hourly.cloud_cover) || []),
      bounds: L.latLngBounds(
        [cLat - halfH, cLon - halfW],
        [cLat + halfH, cLon + halfW]
      ),
    };
    cloudLoading = false;
    const banner = document.getElementById('aurora-debug');
    if (banner) banner.remove();
    renderCloudFrame(cloudHourIdx);
  }

  function renderCloudFrame(hourIdx) {
    if (!cloudGrid || !state.map) return;
    cloudHourIdx = hourIdx;
    const { lats, lons, hourly } = cloudGrid;

    const size = state.map.getSize();
    const W = Math.max(512, Math.round(size.x));
    const H = Math.max(384, Math.round(size.y));
    const canvas = document.createElement('canvas');
    canvas.width = W; canvas.height = H;
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, W, H);

    // Compute pixel coords + values for this hour.
    const pts = [];
    for (let i = 0; i < lats.length; i++) {
      const px = state.map.latLngToContainerPoint([lats[i], lons[i]]);
      const val = hourly[i] && hourly[i].length > hourIdx ? hourly[i][hourIdx] : null;
      if (val == null) continue;
      pts.push({ x: px.x, y: px.y, v: val / 100 });     // v ∈ 0..1
    }

    // Radius sized so neighbors overlap → smooth field. Roughly half the
    // diagonal distance between adjacent grid points.
    const radius = Math.max(W, H) / 6;

    // Paint soft white radial gradients, alpha-scaled by cloud fraction.
    // Using 'lighter' composite so overlapping cloud patches brighten
    // additively (mimicking density).
    ctx.globalCompositeOperation = 'lighter';
    for (const p of pts) {
      const a = Math.min(0.55, p.v * 0.8);
      if (a < 0.02) continue;
      const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, radius);
      g.addColorStop(0,   `rgba(235, 240, 250, ${a})`);
      g.addColorStop(0.5, `rgba(235, 240, 250, ${a * 0.5})`);
      g.addColorStop(1,   'rgba(235, 240, 250, 0)');
      ctx.fillStyle = g;
      ctx.fillRect(p.x - radius, p.y - radius, radius * 2, radius * 2);
    }
    ctx.globalCompositeOperation = 'source-over';

    const vb = state.map.getBounds();
    const dataUrl = canvas.toDataURL('image/png');
    const newOverlay = L.imageOverlay(dataUrl,
      [[vb.getSouth(), vb.getWest()], [vb.getNorth(), vb.getEast()]],
      { opacity: 1, interactive: false, className: 'aurora-cloud-overlay' }
    );

    // Swap overlays in place to avoid flicker.
    const prev = state.overlayLayer;
    newOverlay.addTo(state.map);
    state.overlayLayer = newOverlay;
    if (prev && state.map.hasLayer(prev)) state.map.removeLayer(prev);
  }

  // -------- NASA GIBS satellite tiles (clouds / temp / snow) ----------
  // GIBS offers real global imagery layers as WMTS tiles, free & key-less.
  // Use yesterday's date so the processing pipeline has finished.
  function gibsDate(offsetDays = 1) {
    const d = new Date(Date.now() - offsetDays * 86400_000);
    return d.toISOString().slice(0, 10); // YYYY-MM-DD
  }

  function buildGibsLayer(layerId, maxLevel, fmt, opts) {
    opts = opts || {};
    const date = opts.date || gibsDate(1);
    const url = `https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/${layerId}/default/${date}/GoogleMapsCompatible_Level${maxLevel}/{z}/{y}/{x}.${fmt}`;

    // Dedicated pane so we can set a CSS blend mode on the tile layer. The
    // labels pane is moved above this via addLabelsOnTop().
    const paneName = 'aurora-gibs-' + (opts.blend || 'normal');
    if (!state.map.getPane(paneName)) {
      const pane = state.map.createPane(paneName);
      pane.style.zIndex = 360;
      pane.style.pointerEvents = 'none';
      if (opts.blend) pane.style.mixBlendMode = opts.blend;
    }

    return L.tileLayer(url, {
      maxZoom: 19,
      maxNativeZoom: maxLevel,
      opacity: opts.opacity != null ? opts.opacity : 0.72,
      attribution: 'NASA GIBS',
      crossOrigin: true,
      pane: paneName,
      errorTileUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
    });
  }

  // Given a timeline hour offset (0..239, where 0 = now and larger = further
  // back in the past), produce a YYYY-MM-DD string for GIBS daily imagery.
  function gibsDateFromHourOffset(h) {
    const daysBack = Math.floor(h / 24);
    const d = new Date(Date.now() - (1 + daysBack) * 86400_000);
    return d.toISOString().slice(0, 10);
  }

  let lastCloudDate = null;
  function updateCloudsForTime(hourOffset) {
    const date = gibsDateFromHourOffset(hourOffset);
    if (date === lastCloudDate) return;
    lastCloudDate = date;
    if (state.overlayLayer && state.map.hasLayer(state.overlayLayer)) {
      state.map.removeLayer(state.overlayLayer);
    }
    state.overlayLayer = buildGibsLayer(
      'MODIS_Terra_CorrectedReflectance_TrueColor',
      9, 'jpg', { opacity: 0.9, blend: 'screen', date }
    ).addTo(state.map);
  }

  // -------- real-data grid overlay (Open-Meteo point sampling) --------
  // For layers without free tile services, sample real data on a grid
  // across the current viewport and paint a soft heatmap overlay.
  const GRID_ROWS = 8, GRID_COLS = 14;
  let gridRefreshTimer = null;
  let gridAbortCtrl = null;

  function scheduleGridRefresh(kind) {
    if (state.map && !state._gridHook) {
      state._gridHook = true;
      state.map.on('moveend zoomend', () => {
        const layer = state.currentLayer;
        if (layer === 'clouds') {
          // Re-paint from cache; refetch only if viewport drifted far
          if (cloudGrid && !cloudGrid.bounds.contains(state.map.getBounds().getCenter())) {
            cloudGrid = null;      // force refetch
          }
          loadAndRenderClouds();
          return;
        }
        if (['temp','pressure','waves'].includes(layer)) {
          clearTimeout(gridRefreshTimer);
          if (gridAbortCtrl) gridAbortCtrl.abort();
          gridRefreshTimer = setTimeout(async () => {
            const newLayer = await buildGridDataOverlay(layer);
            if (newLayer && state.currentLayer === layer && state.map) {
              if (state.overlayLayer && state.map.hasLayer(state.overlayLayer)) {
                state.map.removeLayer(state.overlayLayer);
              }
              state.overlayLayer = newLayer.addTo(state.map);
            }
          }, 700);
        }
      });
    }
  }

  async function buildGridDataOverlay(kind) {
    if (!state.map) return null;
    const bounds = state.map.getBounds();

    // Expand the sampled area beyond the visible viewport so the map shows
    // real spatial variation even when zoomed in on a region with uniform
    // weather. Pad by 1.5× in each direction.
    const pad = 1.5;
    const cLat = (bounds.getNorth() + bounds.getSouth()) / 2;
    const cLon = (bounds.getWest() + bounds.getEast()) / 2;
    const halfH = (bounds.getNorth() - bounds.getSouth()) / 2 * (1 + pad);
    const halfW = (bounds.getEast() - bounds.getWest()) / 2 * (1 + pad);
    const north = Math.min(85,  cLat + halfH);
    const south = Math.max(-85, cLat - halfH);
    const west  = cLon - halfW;
    const east  = cLon + halfW;

    const lats = [], lons = [];
    for (let r = 0; r < GRID_ROWS; r++) {
      for (let c = 0; c < GRID_COLS; c++) {
        const lat = north - (r + 0.5) * (north - south) / GRID_ROWS;
        let lon = west + (c + 0.5) * (east - west) / GRID_COLS;
        lon = ((lon + 540) % 360) - 180; // wrap to [-180, 180]
        lats.push(Math.max(-85, Math.min(85, lat)).toFixed(3));
        lons.push(lon.toFixed(3));
      }
    }

    const paramByKind = {
      temp:     { endpoint: 'forecast',         param: 'temperature_2m' },
      clouds:   { endpoint: 'forecast',         param: 'cloud_cover' },
      pressure: { endpoint: 'forecast',         param: 'surface_pressure' },
      snow:     { endpoint: 'forecast',         param: 'snow_depth' },
      waves:    { endpoint: 'marine',           param: 'wave_height' },
    };
    const cfg = paramByKind[kind];
    if (!cfg) return null;

    const host = cfg.endpoint === 'marine'
      ? 'https://marine-api.open-meteo.com/v1/marine'
      : 'https://api.open-meteo.com/v1/forecast';
    const url = `${host}?latitude=${lats.join(',')}&longitude=${lons.join(',')}&current=${cfg.param}&timezone=UTC`;

    // Cancel any previous in-flight grid fetch.
    if (gridAbortCtrl) gridAbortCtrl.abort();
    gridAbortCtrl = new AbortController();
    const signal = gridAbortCtrl.signal;

    let results;
    try {
      const r = await fetch(url, { signal });
      results = await r.json();
    } catch (e) {
      if (e.name !== 'AbortError') console.warn('grid fetch failed', e);
      return null;
    }
    if (!Array.isArray(results)) results = [results];

    // Log a summary so you can verify it's real data in DevTools.
    const vals = results.map(x => x && x.current ? x.current[cfg.param] : null).filter(v => v != null);
    if (vals.length) {
      const mn = Math.min(...vals), mx = Math.max(...vals);
      const avg = vals.reduce((a,b) => a+b, 0) / vals.length;
      console.log(`[AuroraCast] ${kind} (${cfg.param}): ${vals.length} points, min=${mn.toFixed(1)} max=${mx.toFixed(1)} avg=${avg.toFixed(1)}`);
    } else {
      console.warn('[AuroraCast] grid returned no usable values for', kind);
    }

    // Paint onto a canvas sized to the map pane, interpolating between samples.
    const size = state.map.getSize();
    const W = Math.max(256, Math.min(1280, Math.round(size.x)));
    const H = Math.max(256, Math.min(900,  Math.round(size.y)));
    const canvas = document.createElement('canvas');
    canvas.width = W; canvas.height = H;
    const ctx = canvas.getContext('2d');
    const img = ctx.createImageData(W, H);

    // Precompute pixel positions for each grid sample
    const pts = [];
    results.forEach((res, i) => {
      const lat = parseFloat(lats[i]);
      const lon = parseFloat(lons[i]);
      const val = res && res.current ? res.current[cfg.param] : null;
      if (val == null || Number.isNaN(val)) return;
      const px = state.map.latLngToContainerPoint([lat, lon]);
      pts.push({ x: px.x, y: px.y, val });
    });
    if (!pts.length) return null;

    // Inverse-distance-weighted interpolation at each pixel (cheap & smooth).
    // Downsample the canvas by 4 for speed, then upscale via CSS smoothing.
    const STEP = 4;
    for (let y = 0; y < H; y += STEP) {
      for (let x = 0; x < W; x += STEP) {
        let num = 0, den = 0;
        for (const p of pts) {
          const dx = x - p.x, dy = y - p.y;
          const d2 = dx*dx + dy*dy + 1;
          const w = 1 / (d2 * d2); // power 4 IDW → sharper local influence
          num += p.val * w;
          den += w;
        }
        const v = num / den;
        const rgb = colorForValue(kind, v);
        for (let sy = 0; sy < STEP && y + sy < H; sy++) {
          for (let sx = 0; sx < STEP && x + sx < W; sx++) {
            const i = ((y + sy) * W + (x + sx)) * 4;
            img.data[i] = rgb[0];
            img.data[i+1] = rgb[1];
            img.data[i+2] = rgb[2];
            img.data[i+3] = 170;
          }
        }
      }
    }
    ctx.putImageData(img, 0, 0);

    // The canvas was sized to the visible viewport (state.map.getSize()),
    // so the overlay must be anchored to the VISIBLE bounds, not the padded
    // ones used for sampling.
    const dataUrl = canvas.toDataURL('image/png');
    return L.imageOverlay(dataUrl,
      [[bounds.getSouth(), bounds.getWest()], [bounds.getNorth(), bounds.getEast()]],
      { opacity: 0.75, interactive: false, className: 'aurora-grid-overlay' }
    );
  }

  function colorForValue(kind, v) {
    // Stops: array of [thresholdValue, [r,g,b]]
    const stops = ({
      temp: [                  // °C
        [-40, [42, 10, 107]],
        [-20, [26, 58, 110]],
        [-5,  [74, 163, 255]],
        [5,   [92, 211, 137]],
        [15,  [255, 209, 102]],
        [25,  [255, 159, 28]],
        [35,  [255, 107, 107]],
        [45,  [122, 10, 46]],
      ],
      clouds: [                // 0-100 %
        [0,   [15, 20, 30]],
        [25,  [58, 64, 80]],
        [50,  [138, 146, 163]],
        [75,  [200, 210, 222]],
        [100, [240, 244, 250]],
      ],
      pressure: [              // hPa
        [960,  [90, 58, 202]],
        [990,  [74, 163, 255]],
        [1013, [92, 211, 137]],
        [1025, [255, 209, 102]],
        [1045, [255, 107, 107]],
      ],
      snow: [                  // m
        [0,    [20, 25, 40]],
        [0.01, [80, 100, 140]],
        [0.1,  [170, 190, 220]],
        [0.5,  [230, 240, 255]],
        [2,    [255, 255, 255]],
      ],
      waves: [                 // m
        [0,   [10, 20, 40]],
        [1,   [74, 163, 255]],
        [2.5, [92, 211, 137]],
        [4,   [255, 209, 102]],
        [7,   [255, 107, 107]],
      ],
    })[kind];
    for (let i = 0; i < stops.length - 1; i++) {
      if (v <= stops[i][0]) return stops[i][1];
      if (v <= stops[i+1][0]) {
        const a = stops[i], b = stops[i+1];
        const t = (v - a[0]) / (b[0] - a[0]);
        return [
          a[1][0] + (b[1][0] - a[1][0]) * t,
          a[1][1] + (b[1][1] - a[1][1]) * t,
          a[1][2] + (b[1][2] - a[1][2]) * t,
        ];
      }
    }
    return stops[stops.length - 1][1];
  }

  // -------- legacy procedural gradient overlay (kept as fallback) ------
  function buildGradientOverlay(kind) {
    // Smaller canvas → smaller dataURL → renders reliably in the webview.
    const W = 720, H = 360;
    const c = document.createElement('canvas');
    c.width = W; c.height = H;
    const ctx = c.getContext('2d');
    const img = ctx.createImageData(W, H);
    const seed = (kind.charCodeAt(0) * 17.3) + (kind.charCodeAt(1) * 3.7);
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const nx = x / W, ny = y / H;
        // 4-octave noise
        let v = 0, amp = 1, frq = 1;
        for (let o = 0; o < 4; o++) {
          v += amp * Math.sin((nx * frq * 6.28 + seed) + Math.cos((ny * frq * 6.28 + seed * 0.7)));
          amp *= 0.5; frq *= 2;
        }
        v = (v + 2) / 4;             // normalize to ~0..1
        const lat = 90 - ny * 180;   // -90..+90
        const absLat = Math.abs(lat);
        let k = v;
        if (kind === 'pressure') k = 0.5 + 0.35 * Math.sin((lat / 30) * Math.PI) + 0.2 * v;
        if (kind === 'waves')    k = Math.max(0, absLat / 90 - 0.2) * 0.6 + v * 0.4;
        if (kind === 'snow')     k = Math.max(0, (absLat - 40) / 50) * 0.8 + v * 0.2;
        if (kind === 'temp')     k = (1 - absLat / 90) * 0.8 + 0.1 + (v - 0.5) * 0.25;
        if (kind === 'clouds')   k = 0.15 + v * 0.85;
        const rgb = gradient(Math.max(0, Math.min(1, k)), kind);
        const i = (y * W + x) * 4;
        img.data[i]     = rgb[0];
        img.data[i + 1] = rgb[1];
        img.data[i + 2] = rgb[2];
        img.data[i + 3] = 215; // noticeably visible alpha
      }
    }
    ctx.putImageData(img, 0, 0);
    return L.imageOverlay(c.toDataURL('image/png'), [[-85, -180], [85, 180]], {
      opacity: 0.85,
      interactive: false,
      className: 'aurora-gradient-overlay',
    });
  }
  function gradient(t, kind) {
    t = Math.max(0, Math.min(1, t));
    const stops = ({
      pressure: [[0, [90, 58, 202]], [.35, [74, 163, 255]], [.55, [92, 211, 137]], [.75, [255, 209, 102]], [1, [255, 107, 107]]],
      waves:    [[0, [10, 20, 40]],  [.3,  [74, 163, 255]], [.6,  [92, 211, 137]], [.85, [255, 209, 102]], [1, [255, 107, 107]]],
      snow:     [[0, [10, 15, 25]],  [.2,  [80, 100, 130]], [.5,  [170, 190, 210]], [.8, [220, 230, 245]],  [1, [255, 255, 255]]],
      temp:     [[0, [42, 10, 107]], [.2,  [26, 58, 110]],  [.4,  [74, 163, 255]],  [.55,[92, 211, 137]],   [.7, [255, 209, 102]], [.85, [255, 107, 107]], [1, [122, 10, 46]]],
      clouds:   [[0, [11, 14, 20]],  [.3,  [58, 64, 80]],   [.6,  [138, 146, 163]], [1,  [230, 234, 242]]],
    })[kind];
    for (let i = 0; i < stops.length - 1; i++) {
      if (t >= stops[i][0] && t <= stops[i + 1][0]) {
        const a = stops[i], b = stops[i + 1];
        const k = (t - a[0]) / (b[0] - a[0]);
        return [a[1][0] + (b[1][0] - a[1][0]) * k,
                a[1][1] + (b[1][1] - a[1][1]) * k,
                a[1][2] + (b[1][2] - a[1][2]) * k];
      }
    }
    return [255, 255, 255];
  }

  // -------- data fetchers (shared across platforms) -------------------
  async function geocode(query) {
    const r = await fetch(
      `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(query)}&count=8&language=en&format=json`
    );
    const j = await r.json();
    return j.results || [];
  }

  async function reverseGeocode(lat, lon) {
    try {
      const r = await fetch(
        `https://geocoding-api.open-meteo.com/v1/reverse?latitude=${lat}&longitude=${lon}&language=en&format=json`
      );
      const j = await r.json();
      return (j.results || [])[0] || null;
    } catch (_) { return null; }
  }

  async function forecast(lat, lon, model) {
    const modelMap = {
      ecmwf: 'ecmwf_ifs025',
      gfs: 'gfs_seamless',
      icon: 'icon_seamless',
      ukmo: 'ukmo_seamless',
    };
    const p = new URLSearchParams({
      latitude: lat, longitude: lon,
      current: 'temperature_2m,apparent_temperature,weather_code,wind_speed_10m,wind_direction_10m,relative_humidity_2m,surface_pressure,cloud_cover,precipitation',
      hourly: 'temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,precipitation_probability,precipitation',
      daily: 'weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum,precipitation_probability_max,wind_speed_10m_max,sunrise,sunset',
      timezone: 'auto',
      forecast_days: 10,
      models: modelMap[model] || 'best_match',
      wind_speed_unit: 'kmh',
      temperature_unit: 'celsius',
    });
    const r = await fetch('https://api.open-meteo.com/v1/forecast?' + p.toString());
    return await r.json();
  }

  async function quickCurrent(lat, lon) {
    const r = await fetch(
      `https://api.open-meteo.com/v1/forecast?latitude=${lat}&longitude=${lon}&current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m&wind_speed_unit=kmh`
    );
    return await r.json();
  }

  // -------- expose -----------------------------------------------------
  window.AuroraMap = {
    init, setLayer, setTime, panTo, showPopup, invalidate,
    geocode, reverseGeocode, forecast, quickCurrent,
  };

  // Smooth timeline drag. To feel responsive we:
  //   1. Update the playhead DOM directly on every pointermove (no roundtrip)
  //   2. Throttle the aurora:timeline event that syncs Rust state (~60 Hz)
  //   3. Always send a final event on pointerup so state is authoritative
  function setupTimelineDrag(track) {
    if (!track || track._auroraDragWired) return;
    track._auroraDragWired = true;

    let dragging = false;
    let lastFrac = 0;
    let lastSyncAt = 0;
    const SYNC_INTERVAL_MS = 60;

    const visualUpdate = (frac) => {
      const head = document.querySelector('.tl-playhead');
      if (head) head.style.left = (frac * 100) + '%';
    };
    const syncToRust = (frac) => {
      window.dispatchEvent(new CustomEvent('aurora:timeline', { detail: frac }));
    };
    const fracOf = (clientX) => {
      const r = track.getBoundingClientRect();
      return Math.max(0, Math.min(1, (clientX - r.left) / r.width));
    };

    track.addEventListener('pointerdown', (e) => {
      dragging = true;
      try { track.setPointerCapture(e.pointerId); } catch (_) {}
      const f = fracOf(e.clientX);
      lastFrac = f;
      visualUpdate(f);
      syncToRust(f);
      lastSyncAt = performance.now();
      e.preventDefault();
    });
    track.addEventListener('pointermove', (e) => {
      if (!dragging) return;
      const f = fracOf(e.clientX);
      lastFrac = f;
      visualUpdate(f);                  // instant visual feedback
      const now = performance.now();
      if (now - lastSyncAt > SYNC_INTERVAL_MS) {
        syncToRust(f);                  // throttled state sync
        lastSyncAt = now;
      }
    });
    const end = (e) => {
      if (!dragging) return;
      dragging = false;
      try { track.releasePointerCapture(e.pointerId); } catch (_) {}
      syncToRust(lastFrac);             // authoritative final value
    };
    track.addEventListener('pointerup', end);
    track.addEventListener('pointercancel', end);
  }
  function watchTimeline() {
    const look = () => {
      const el = document.querySelector('.tl-track');
      if (el) setupTimelineDrag(el);
    };
    // Poll for the element (Rust mounts the DOM asynchronously) and keep
    // polling — Dioxus may re-mount the element.
    setInterval(look, 400);
  }

  // Auto-init: once #map appears in the DOM, initialize without waiting
  // for the Rust side. (The Rust `use_effect` also calls init; whichever
  // fires first wins — `init` is idempotent.)
  function autoInit() {
    const el = document.getElementById('map');
    if (el && !state.map) {
      init('map').catch(err => showDebugBanner('auto-init error: ' + err.message));
      watchTimeline();
      return true;
    }
    return false;
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      const iv = setInterval(() => { if (autoInit()) clearInterval(iv); }, 100);
      setTimeout(() => clearInterval(iv), 20000);
    });
  } else {
    const iv = setInterval(() => { if (autoInit()) clearInterval(iv); }, 100);
    setTimeout(() => clearInterval(iv), 20000);
  }
})();
