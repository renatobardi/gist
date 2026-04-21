use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::web::{middleware::auth::AuthenticatedUser, state::AppState};

#[derive(Deserialize)]
pub struct GraphQueryParams {
    pub domain: Option<String>,
}

pub async fn get_api_graph(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(params): Query<GraphQueryParams>,
) -> Response {
    let domains = params
        .domain
        .map(|d| {
            d.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());

    match state.graph_read_repo.get_graph(domains).await {
        Ok(data) => Json(data).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_error", "message": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_api_concept_by_id(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Response {
    match state.graph_read_repo.get_concept_detail(&id).await {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "message": "Concept not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_error", "message": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_graph_page(
    State(state): State<AppState>,
    auth: Result<AuthenticatedUser, Response>,
) -> Response {
    match auth {
        Ok(_) => Html(GRAPH_HTML).into_response(),
        Err(_) => match state.user_repo.count().await {
            Ok(0) => Redirect::to("/setup").into_response(),
            _ => Redirect::to("/login").into_response(),
        },
    }
}

pub async fn get_concept_detail_page(
    State(state): State<AppState>,
    auth: Result<AuthenticatedUser, Response>,
) -> Response {
    match auth {
        Ok(_) => Html(CONCEPT_DETAIL_HTML).into_response(),
        Err(_) => match state.user_repo.count().await {
            Ok(0) => Redirect::to("/setup").into_response(),
            _ => Redirect::to("/login").into_response(),
        },
    }
}

const GRAPH_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Concept Graph</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Inter, Arial, sans-serif;
      background: #f5f5f5;
      color: #262626;
      margin: 0;
      height: 100vh;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }
    header {
      background: #1a1a1a;
      color: #f0f0f0;
      padding: 0 2rem;
      height: 56px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-shrink: 0;
    }
    header h1 {
      font-size: 1rem;
      font-weight: 700;
      margin: 0;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: #f0f0f0;
    }
    .header-nav {
      display: flex;
      align-items: center;
      gap: 1.5rem;
    }
    .nav-link {
      color: #8c8c8c;
      text-decoration: none;
      font-size: 0.875rem;
    }
    .nav-link:hover { color: #f0f0f0; }
    .nav-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .toolbar {
      background: #fff;
      border-bottom: 1px solid #e5e5e5;
      padding: 0.6rem 1.5rem;
      display: flex;
      align-items: center;
      gap: 1rem;
      flex-wrap: wrap;
      flex-shrink: 0;
    }
    .toolbar-label {
      font-size: 0.8rem;
      font-weight: 600;
      color: #595959;
      white-space: nowrap;
    }
    .domain-chips {
      display: flex;
      flex-wrap: wrap;
      gap: 0.4rem;
      flex: 1;
    }
    .chip {
      padding: 0.25rem 0.65rem;
      font-size: 0.75rem;
      font-weight: 600;
      border: 1px solid #e5e5e5;
      background: #f5f5f5;
      color: #595959;
      cursor: pointer;
      user-select: none;
      transition: background 0.1s;
    }
    .chip:hover { background: #e5e5e5; }
    .chip.active {
      background: #1c69d4;
      color: #fff;
      border-color: #1c69d4;
    }
    .chip:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .zoom-controls {
      display: flex;
      gap: 0.35rem;
      align-items: center;
      margin-left: auto;
    }
    .zoom-btn {
      width: 30px;
      height: 30px;
      background: #f5f5f5;
      border: 1px solid #e5e5e5;
      color: #262626;
      font-size: 1rem;
      line-height: 1;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      font-family: inherit;
    }
    .zoom-btn:hover { background: #e5e5e5; }
    .zoom-btn:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .graph-container {
      flex: 1;
      position: relative;
      overflow: hidden;
      min-height: 0;
    }
    #graph-canvas {
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      cursor: grab;
    }
    #graph-canvas.dragging { cursor: grabbing; }
    #graph-canvas.node-drag { cursor: grabbing; }
    .tooltip {
      position: absolute;
      background: #1a1a1a;
      color: #f0f0f0;
      padding: 0.4rem 0.7rem;
      font-size: 0.8rem;
      font-weight: 600;
      pointer-events: none;
      max-width: 200px;
      word-break: break-word;
      z-index: 10;
      display: none;
    }
    .tooltip .tooltip-domain {
      font-size: 0.7rem;
      color: #8c8c8c;
      font-weight: 400;
      margin-top: 0.15rem;
    }
    .loading-overlay {
      position: absolute;
      inset: 0;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #f5f5f5;
      font-size: 1rem;
      color: #595959;
      z-index: 20;
    }
    .error-overlay {
      position: absolute;
      inset: 0;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #f5f5f5;
      z-index: 20;
    }
    .error-box {
      background: #f8d7da;
      border: 1px solid #dc3545;
      color: #721c24;
      padding: 1rem 1.5rem;
      font-size: 0.9rem;
      max-width: 400px;
      text-align: center;
    }
    .empty-overlay {
      position: absolute;
      inset: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      background: #f5f5f5;
      z-index: 20;
      color: #595959;
      gap: 0.5rem;
    }
    .empty-overlay p { margin: 0; font-size: 1rem; }
    .empty-overlay .sub { font-size: 0.875rem; color: #8c8c8c; }
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <nav class="header-nav" aria-label="Site navigation">
      <a href="/" class="nav-link">Library</a>
      <a href="/graph" class="nav-link" aria-current="page" style="color:#f0f0f0;">Graph</a>
    </nav>
  </header>

  <div class="toolbar" role="toolbar" aria-label="Graph filters and controls">
    <span class="toolbar-label">Domain</span>
    <div class="domain-chips" id="domain-chips" role="group" aria-label="Domain filters">
      <button class="chip active" data-domain="all" role="checkbox" aria-checked="true">All</button>
    </div>
    <div class="zoom-controls" aria-label="Zoom controls">
      <button class="zoom-btn" id="zoom-in" aria-label="Zoom in" title="Zoom in">+</button>
      <button class="zoom-btn" id="zoom-out" aria-label="Zoom out" title="Zoom out">−</button>
      <button class="zoom-btn" id="zoom-reset" aria-label="Reset zoom" title="Reset view" style="font-size:0.75rem;width:auto;padding:0 0.5rem;">Fit</button>
    </div>
  </div>

  <div class="graph-container" role="main" aria-label="Concept graph visualization">
    <canvas id="graph-canvas" tabindex="0" aria-label="Interactive concept graph. Click nodes to view details."></canvas>
    <div class="tooltip" id="tooltip" role="tooltip"></div>
    <div class="loading-overlay" id="loading" aria-live="polite">Loading graph…</div>
  </div>

  <script>
    /* ── State ─────────────────────────────────────────────────── */
    var canvas = document.getElementById('graph-canvas');
    var ctx = canvas.getContext('2d');
    var tooltip = document.getElementById('tooltip');
    var loading = document.getElementById('loading');

    var allNodes = [], allEdges = [];
    var nodes = [], edges = [];
    var activeDomains = new Set(); // empty = all
    var uniqueDomains = [];

    var scale = 1;
    var offset = { x: 0, y: 0 };
    var isPanning = false;
    var panStart = { x: 0, y: 0 };
    var dragNode = null;
    var dragOffset = { x: 0, y: 0 };
    var hoveredNode = null;

    var NODE_W = 120, NODE_H = 36;

    /* ── Domain colours ─────────────────────────────────────────── */
    var DOMAIN_PALETTE = [
      '#1c69d4','#28a745','#6f42c1','#fd7e14',
      '#20c997','#e83e8c','#17a2b8','#dc3545',
      '#6610f2','#795548','#607d8b','#ff5722',
    ];
    var domainColorMap = {};
    function domainColor(domain) {
      if (!domain) return '#8c8c8c';
      if (!domainColorMap[domain]) {
        var idx = Object.keys(domainColorMap).length % DOMAIN_PALETTE.length;
        domainColorMap[domain] = DOMAIN_PALETTE[idx];
      }
      return domainColorMap[domain];
    }

    /* ── Layout: force-directed ─────────────────────────────────── */
    function initPositions(nodeList) {
      var cx = canvas.width / (2 * scale);
      var cy = canvas.height / (2 * scale);
      var r = Math.min(canvas.width, canvas.height) * 0.35 / scale;
      nodeList.forEach(function(n, i) {
        if (n.x == null || n.y == null) {
          var angle = (2 * Math.PI * i) / nodeList.length;
          n.x = cx + r * Math.cos(angle);
          n.y = cy + r * Math.sin(angle);
        }
        n.vx = 0;
        n.vy = 0;
      });
    }

    var SIM_STEPS = 120;
    var simStep = 0;
    var simTimer = null;

    function runSimulation() {
      if (simTimer) clearInterval(simTimer);
      simStep = 0;
      simTimer = setInterval(function() {
        if (simStep >= SIM_STEPS) { clearInterval(simTimer); return; }
        simulate();
        render();
        simStep++;
      }, 16);
    }

    function simulate() {
      var k = 80; // repulsion constant
      var spring = 0.05;
      var ideal = 200; // ideal edge length

      // Repulsion
      for (var i = 0; i < nodes.length; i++) {
        for (var j = i + 1; j < nodes.length; j++) {
          var dx = nodes[j].x - nodes[i].x;
          var dy = nodes[j].y - nodes[i].y;
          var dist = Math.sqrt(dx * dx + dy * dy) || 1;
          var force = k * k / dist;
          nodes[i].vx -= force * dx / dist;
          nodes[i].vy -= force * dy / dist;
          nodes[j].vx += force * dx / dist;
          nodes[j].vy += force * dy / dist;
        }
      }

      // Attraction along edges
      edges.forEach(function(e) {
        var src = nodeById[e.source];
        var tgt = nodeById[e.target];
        if (!src || !tgt) return;
        var dx = tgt.x - src.x;
        var dy = tgt.y - src.y;
        var dist = Math.sqrt(dx * dx + dy * dy) || 1;
        var force = spring * (dist - ideal);
        src.vx += force * dx / dist;
        src.vy += force * dy / dist;
        tgt.vx -= force * dx / dist;
        tgt.vy -= force * dy / dist;
      });

      // Damping + apply
      var damp = 0.85;
      nodes.forEach(function(n) {
        if (n === dragNode) return;
        n.vx *= damp;
        n.vy *= damp;
        n.x += n.vx;
        n.y += n.vy;
      });
    }

    /* ── Render ─────────────────────────────────────────────────── */
    function render() {
      var dpr = window.devicePixelRatio || 1;
      var w = canvas.clientWidth;
      var h = canvas.clientHeight;
      if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        ctx.scale(dpr, dpr);
      }

      ctx.clearRect(0, 0, w, h);
      ctx.save();
      ctx.translate(offset.x, offset.y);
      ctx.scale(scale, scale);

      // Edges
      edges.forEach(function(e) {
        var src = nodeById[e.source];
        var tgt = nodeById[e.target];
        if (!src || !tgt) return;
        var thickness = 1 + e.strength * 2;
        ctx.strokeStyle = 'rgba(180,180,180,0.6)';
        ctx.lineWidth = thickness;
        drawOrthoEdge(src, tgt);
      });

      // Nodes
      nodes.forEach(function(n) {
        var isHovered = n === hoveredNode;
        var color = domainColor(n.domain);
        var x = n.x - NODE_W / 2;
        var y = n.y - NODE_H / 2;

        ctx.fillStyle = isHovered ? '#f0f0f0' : '#fff';
        ctx.fillRect(x, y, NODE_W, NODE_H);

        ctx.strokeStyle = isHovered ? '#1c69d4' : color;
        ctx.lineWidth = isHovered ? 2 : 1.5;
        ctx.strokeRect(x, y, NODE_W, NODE_H);

        ctx.fillStyle = '#262626';
        ctx.font = '600 11px -apple-system, BlinkMacSystemFont, Arial, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        var label = n.display_name || n.name;
        if (label.length > 16) label = label.substring(0, 15) + '…';
        ctx.fillText(label, n.x, n.y);
      });

      ctx.restore();
    }

    function drawOrthoEdge(src, tgt) {
      var sx = src.x, sy = src.y;
      var tx = tgt.x, ty = tgt.y;
      var midY = (sy + ty) / 2;
      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.lineTo(sx, midY);
      ctx.lineTo(tx, midY);
      ctx.lineTo(tx, ty);
      ctx.stroke();
    }

    /* ── Filtering ──────────────────────────────────────────────── */
    var nodeById = {};

    function applyFilter() {
      if (activeDomains.size === 0) {
        nodes = allNodes.slice();
      } else {
        nodes = allNodes.filter(function(n) { return activeDomains.has(n.domain); });
      }
      var nodeIdSet = new Set(nodes.map(function(n) { return n.id; }));
      edges = allEdges.filter(function(e) {
        return nodeIdSet.has(e.source) && nodeIdSet.has(e.target);
      });
      nodeById = {};
      nodes.forEach(function(n) { nodeById[n.id] = n; });
      initPositions(nodes);
      runSimulation();
    }

    /* ── Domain chips UI ────────────────────────────────────────── */
    function buildDomainChips() {
      var container = document.getElementById('domain-chips');
      container.innerHTML = '';

      var allChip = document.createElement('button');
      allChip.className = 'chip' + (activeDomains.size === 0 ? ' active' : '');
      allChip.setAttribute('data-domain', 'all');
      allChip.setAttribute('role', 'checkbox');
      allChip.setAttribute('aria-checked', activeDomains.size === 0 ? 'true' : 'false');
      allChip.textContent = 'All';
      allChip.addEventListener('click', function() {
        activeDomains.clear();
        buildDomainChips();
        applyFilter();
      });
      container.appendChild(allChip);

      uniqueDomains.forEach(function(d) {
        if (!d) return;
        var chip = document.createElement('button');
        var isActive = activeDomains.has(d);
        chip.className = 'chip' + (isActive ? ' active' : '');
        chip.setAttribute('data-domain', d);
        chip.setAttribute('role', 'checkbox');
        chip.setAttribute('aria-checked', isActive ? 'true' : 'false');
        chip.textContent = d;
        chip.addEventListener('click', function() {
          if (activeDomains.has(d)) {
            activeDomains.delete(d);
          } else {
            activeDomains.add(d);
          }
          buildDomainChips();
          applyFilter();
        });
        container.appendChild(chip);
      });
    }

    /* ── Canvas interaction ─────────────────────────────────────── */
    function canvasToWorld(cx, cy) {
      return {
        x: (cx - offset.x) / scale,
        y: (cy - offset.y) / scale,
      };
    }

    function hitTest(wx, wy) {
      for (var i = nodes.length - 1; i >= 0; i--) {
        var n = nodes[i];
        if (wx >= n.x - NODE_W / 2 && wx <= n.x + NODE_W / 2 &&
            wy >= n.y - NODE_H / 2 && wy <= n.y + NODE_H / 2) {
          return n;
        }
      }
      return null;
    }

    canvas.addEventListener('mousedown', function(e) {
      var rect = canvas.getBoundingClientRect();
      var cx = e.clientX - rect.left;
      var cy = e.clientY - rect.top;
      var w = canvasToWorld(cx, cy);
      var hit = hitTest(w.x, w.y);
      if (hit) {
        dragNode = hit;
        dragOffset.x = hit.x - w.x;
        dragOffset.y = hit.y - w.y;
        canvas.classList.add('node-drag');
      } else {
        isPanning = true;
        panStart.x = e.clientX - offset.x;
        panStart.y = e.clientY - offset.y;
        canvas.classList.add('dragging');
      }
    });

    canvas.addEventListener('mousemove', function(e) {
      var rect = canvas.getBoundingClientRect();
      var cx = e.clientX - rect.left;
      var cy = e.clientY - rect.top;

      if (dragNode) {
        var w = canvasToWorld(cx, cy);
        dragNode.x = w.x + dragOffset.x;
        dragNode.y = w.y + dragOffset.y;
        dragNode.vx = 0;
        dragNode.vy = 0;
        render();
        return;
      }

      if (isPanning) {
        offset.x = e.clientX - panStart.x;
        offset.y = e.clientY - panStart.y;
        render();
        return;
      }

      var w = canvasToWorld(cx, cy);
      var hit = hitTest(w.x, w.y);
      if (hit !== hoveredNode) {
        hoveredNode = hit;
        render();
      }
      if (hit) {
        tooltip.style.display = 'block';
        tooltip.style.left = (cx + 12) + 'px';
        tooltip.style.top = (cy - 8) + 'px';
        tooltip.innerHTML =
          '<div>' + escapeHtml(hit.display_name || hit.name) + '</div>' +
          (hit.domain ? '<div class="tooltip-domain">' + escapeHtml(hit.domain) + '</div>' : '');
      } else {
        tooltip.style.display = 'none';
      }
    });

    canvas.addEventListener('mouseup', function(e) {
      var wasDrag = !!dragNode;
      var wasPan = isPanning;
      dragNode = null;
      isPanning = false;
      canvas.classList.remove('dragging', 'node-drag');

      if (!wasDrag && !wasPan) return;
      if (wasDrag) {
        // check if it was actually a click (no movement)
        var rect = canvas.getBoundingClientRect();
        var cx = e.clientX - rect.left;
        var cy = e.clientY - rect.top;
        var w = canvasToWorld(cx, cy);
        var hit = hitTest(w.x, w.y);
        if (hit) {
          window.location.href = '/graph/concepts/' + encodeURIComponent(hit.id);
        }
      }
    });

    canvas.addEventListener('click', function(e) {
      if (dragNode || isPanning) return;
      var rect = canvas.getBoundingClientRect();
      var cx = e.clientX - rect.left;
      var cy = e.clientY - rect.top;
      var w = canvasToWorld(cx, cy);
      var hit = hitTest(w.x, w.y);
      if (hit) {
        window.location.href = '/graph/concepts/' + encodeURIComponent(hit.id);
      }
    });

    canvas.addEventListener('mouseleave', function() {
      hoveredNode = null;
      tooltip.style.display = 'none';
      isPanning = false;
      dragNode = null;
      canvas.classList.remove('dragging', 'node-drag');
    });

    canvas.addEventListener('wheel', function(e) {
      e.preventDefault();
      var rect = canvas.getBoundingClientRect();
      var cx = e.clientX - rect.left;
      var cy = e.clientY - rect.top;
      var delta = e.deltaY > 0 ? 0.85 : 1.15;
      var newScale = Math.max(0.1, Math.min(5, scale * delta));
      offset.x = cx - (cx - offset.x) * (newScale / scale);
      offset.y = cy - (cy - offset.y) * (newScale / scale);
      scale = newScale;
      render();
    }, { passive: false });

    /* ── Zoom buttons ───────────────────────────────────────────── */
    document.getElementById('zoom-in').addEventListener('click', function() {
      scale = Math.min(5, scale * 1.2);
      render();
    });
    document.getElementById('zoom-out').addEventListener('click', function() {
      scale = Math.max(0.1, scale / 1.2);
      render();
    });
    document.getElementById('zoom-reset').addEventListener('click', function() {
      fitGraph();
      render();
    });

    function fitGraph() {
      if (nodes.length === 0) { scale = 1; offset = { x: 0, y: 0 }; return; }
      var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      nodes.forEach(function(n) {
        minX = Math.min(minX, n.x - NODE_W / 2);
        minY = Math.min(minY, n.y - NODE_H / 2);
        maxX = Math.max(maxX, n.x + NODE_W / 2);
        maxY = Math.max(maxY, n.y + NODE_H / 2);
      });
      var pad = 40;
      var gw = maxX - minX + pad * 2;
      var gh = maxY - minY + pad * 2;
      var cw = canvas.clientWidth;
      var ch = canvas.clientHeight;
      scale = Math.min(cw / gw, ch / gh, 2);
      offset.x = (cw - gw * scale) / 2 - (minX - pad) * scale;
      offset.y = (ch - gh * scale) / 2 - (minY - pad) * scale;
    }

    /* ── Keyboard accessibility ─────────────────────────────────── */
    canvas.addEventListener('keydown', function(e) {
      if (e.key === 'Enter' || e.key === ' ') {
        if (hoveredNode) {
          window.location.href = '/graph/concepts/' + encodeURIComponent(hoveredNode.id);
        }
      }
    });

    /* ── Utilities ──────────────────────────────────────────────── */
    function escapeHtml(str) {
      if (!str) return '';
      return String(str)
        .replace(/&/g, '&amp;').replace(/</g, '&lt;')
        .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    /* ── Load data ──────────────────────────────────────────────── */
    function loadGraph() {
      fetch('/api/graph', { credentials: 'same-origin' })
        .then(function(res) {
          if (!res.ok) throw new Error('HTTP ' + res.status);
          return res.json();
        })
        .then(function(data) {
          loading.style.display = 'none';

          allNodes = (data.nodes || []).map(function(n) {
            return { id: n.id, name: n.name, display_name: n.display_name, domain: n.domain, x: null, y: null, vx: 0, vy: 0 };
          });
          allEdges = data.edges || [];

          if (allNodes.length === 0) {
            var graphContainer = canvas.parentElement;
            var empty = document.createElement('div');
            empty.className = 'empty-overlay';
            empty.innerHTML = '<p>No concepts yet</p><p class="sub">Add books and process them to populate the concept graph.</p>';
            graphContainer.appendChild(empty);
            return;
          }

          // collect unique domains, preserving order by first appearance
          var domainSeen = {};
          allNodes.forEach(function(n) {
            if (n.domain && !domainSeen[n.domain]) {
              domainSeen[n.domain] = true;
              uniqueDomains.push(n.domain);
              domainColor(n.domain);
            }
          });

          buildDomainChips();
          applyFilter();

          // Fit after simulation settles
          setTimeout(function() {
            fitGraph();
            render();
          }, SIM_STEPS * 16 + 100);
        })
        .catch(function(err) {
          loading.style.display = 'none';
          var graphContainer = canvas.parentElement;
          var errDiv = document.createElement('div');
          errDiv.className = 'error-overlay';
          errDiv.innerHTML = '<div class="error-box">Failed to load graph: ' + escapeHtml(err.message) + '</div>';
          graphContainer.appendChild(errDiv);
        });
    }

    /* ── Init ───────────────────────────────────────────────────── */
    window.addEventListener('resize', function() { render(); });
    loadGraph();
  </script>
</body>
</html>"#;

const CONCEPT_DETAIL_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Concept</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Inter, Arial, sans-serif;
      background: #f5f5f5;
      color: #262626;
      margin: 0;
      min-height: 100vh;
    }
    header {
      background: #1a1a1a;
      color: #f0f0f0;
      padding: 0 2rem;
      height: 56px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    header h1 {
      font-size: 1rem;
      font-weight: 700;
      margin: 0;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: #f0f0f0;
    }
    .header-nav {
      display: flex;
      align-items: center;
      gap: 1.5rem;
    }
    .nav-link {
      color: #8c8c8c;
      text-decoration: none;
      font-size: 0.875rem;
    }
    .nav-link:hover { color: #f0f0f0; }
    .nav-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    main {
      max-width: 860px;
      margin: 2.5rem auto;
      padding: 0 1.5rem;
    }
    .breadcrumb {
      font-size: 0.8rem;
      color: #8c8c8c;
      margin-bottom: 1.25rem;
    }
    .breadcrumb a {
      color: #1c69d4;
      text-decoration: none;
    }
    .breadcrumb a:hover { text-decoration: underline; }
    .breadcrumb a:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .breadcrumb span { margin: 0 0.4rem; }
    .concept-card {
      background: #fff;
      border: 1px solid #e5e5e5;
      border-left: 4px solid #1c69d4;
      padding: 1.5rem;
      margin-bottom: 1.5rem;
    }
    .concept-name {
      font-size: 1.5rem;
      font-weight: 700;
      margin: 0 0 0.25rem;
    }
    .concept-domain {
      display: inline-block;
      padding: 0.2rem 0.6rem;
      background: #e8f0fd;
      color: #1c69d4;
      font-size: 0.75rem;
      font-weight: 600;
      margin-bottom: 0.75rem;
    }
    .concept-description {
      font-size: 0.925rem;
      line-height: 1.6;
      color: #595959;
      margin: 0;
    }
    .section {
      background: #fff;
      border: 1px solid #e5e5e5;
      padding: 1.25rem 1.5rem;
      margin-bottom: 1.25rem;
    }
    .section-title {
      font-size: 0.875rem;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: #595959;
      margin: 0 0 1rem;
    }
    .book-list {
      list-style: none;
      margin: 0;
      padding: 0;
      display: flex;
      flex-direction: column;
      gap: 0.6rem;
    }
    .book-item {
      display: flex;
      align-items: baseline;
      gap: 0.75rem;
    }
    .book-link {
      font-size: 0.925rem;
      font-weight: 600;
      color: #1c69d4;
      text-decoration: none;
    }
    .book-link:hover { text-decoration: underline; }
    .book-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .book-author {
      font-size: 0.8rem;
      color: #8c8c8c;
    }
    .related-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 0.75rem;
    }
    .related-card {
      border: 1px solid #e5e5e5;
      padding: 0.75rem;
      background: #fafafa;
      text-decoration: none;
      color: inherit;
      display: block;
      transition: background 0.1s;
    }
    .related-card:hover { background: #f0f0f0; }
    .related-card:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .related-name {
      font-size: 0.875rem;
      font-weight: 600;
      margin: 0 0 0.2rem;
    }
    .related-meta {
      font-size: 0.75rem;
      color: #8c8c8c;
      display: flex;
      gap: 0.5rem;
    }
    .rel-type {
      text-transform: capitalize;
    }
    .strength-bar {
      display: inline-block;
      width: 40px;
      height: 4px;
      background: #e5e5e5;
      vertical-align: middle;
      position: relative;
      margin-left: 0.25rem;
    }
    .strength-fill {
      position: absolute;
      left: 0;
      top: 0;
      height: 100%;
      background: #1c69d4;
    }
    .empty-section {
      font-size: 0.875rem;
      color: #8c8c8c;
      font-style: italic;
    }
    .loading-state {
      text-align: center;
      padding: 4rem 1rem;
      color: #595959;
    }
    .error-state {
      background: #f8d7da;
      border: 1px solid #dc3545;
      color: #721c24;
      padding: 1rem;
      font-size: 0.9rem;
    }
    .back-link {
      display: inline-flex;
      align-items: center;
      gap: 0.3rem;
      color: #1c69d4;
      text-decoration: none;
      font-size: 0.875rem;
      margin-bottom: 1rem;
      display: block;
    }
    .back-link:hover { text-decoration: underline; }
    .back-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <nav class="header-nav" aria-label="Site navigation">
      <a href="/" class="nav-link">Library</a>
      <a href="/graph" class="nav-link">Graph</a>
    </nav>
  </header>
  <main>
    <nav class="breadcrumb" aria-label="Breadcrumb">
      <a href="/graph">Graph</a>
      <span aria-hidden="true">›</span>
      <span id="breadcrumb-name">Concept</span>
    </nav>
    <div id="content">
      <div class="loading-state" aria-live="polite">Loading…</div>
    </div>
  </main>

  <script>
    var content = document.getElementById('content');

    function escapeHtml(str) {
      if (!str) return '';
      return String(str)
        .replace(/&/g, '&amp;').replace(/</g, '&lt;')
        .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    function getConceptId() {
      var parts = window.location.pathname.split('/');
      return decodeURIComponent(parts[parts.length - 1]);
    }

    function renderDetail(data) {
      var c = data.concept;
      document.title = 'Knowledge Vault — ' + (c.display_name || c.name);
      document.getElementById('breadcrumb-name').textContent = c.display_name || c.name;

      var booksHtml = '';
      if (!data.books || data.books.length === 0) {
        booksHtml = '<p class="empty-section">No books linked yet.</p>';
      } else {
        booksHtml = '<ul class="book-list" role="list">';
        data.books.forEach(function(b) {
          booksHtml +=
            '<li class="book-item">' +
              '<a href="/works/' + escapeHtml(b.work_id) + '" class="book-link">' + escapeHtml(b.title) + '</a>' +
              (b.author ? '<span class="book-author">by ' + escapeHtml(b.author) + '</span>' : '') +
            '</li>';
        });
        booksHtml += '</ul>';
      }

      var relatedHtml = '';
      if (!data.related_concepts || data.related_concepts.length === 0) {
        relatedHtml = '<p class="empty-section">No related concepts found.</p>';
      } else {
        relatedHtml = '<div class="related-grid">';
        data.related_concepts.forEach(function(r) {
          var pct = Math.round((r.strength || 0) * 100);
          relatedHtml +=
            '<a href="/graph/concepts/' + encodeURIComponent(r.id) + '" class="related-card">' +
              '<p class="related-name">' + escapeHtml(r.display_name || r.id) + '</p>' +
              '<div class="related-meta">' +
                '<span class="rel-type">' + escapeHtml(r.relation_type || 'related') + '</span>' +
                '<span>' +
                  '<span class="strength-bar" aria-label="Strength ' + pct + '%">' +
                    '<span class="strength-fill" style="width:' + pct + '%"></span>' +
                  '</span>' +
                '</span>' +
              '</div>' +
            '</a>';
        });
        relatedHtml += '</div>';
      }

      content.innerHTML =
        '<div class="concept-card">' +
          '<h2 class="concept-name">' + escapeHtml(c.display_name || c.name) + '</h2>' +
          (c.domain ? '<span class="concept-domain">' + escapeHtml(c.domain) + '</span>' : '') +
          (c.description ? '<p class="concept-description">' + escapeHtml(c.description) + '</p>' : '') +
        '</div>' +
        '<section class="section" aria-label="Books mentioning this concept">' +
          '<h3 class="section-title">Books (' + (data.books ? data.books.length : 0) + ')</h3>' +
          booksHtml +
        '</section>' +
        '<section class="section" aria-label="Related concepts">' +
          '<h3 class="section-title">Related Concepts (' + (data.related_concepts ? data.related_concepts.length : 0) + ')</h3>' +
          relatedHtml +
        '</section>';
    }

    function loadConcept() {
      var id = getConceptId();
      fetch('/api/concepts/' + encodeURIComponent(id), { credentials: 'same-origin' })
        .then(function(res) {
          if (res.status === 404) throw new Error('Concept not found');
          if (!res.ok) throw new Error('HTTP ' + res.status);
          return res.json();
        })
        .then(function(data) {
          renderDetail(data);
        })
        .catch(function(err) {
          content.innerHTML =
            '<div class="error-state" role="alert">Failed to load concept: ' + escapeHtml(err.message) + '</div>';
        });
    }

    loadConcept();
  </script>
</body>
</html>"#;
