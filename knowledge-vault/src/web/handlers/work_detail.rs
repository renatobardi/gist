use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
};
use serde_json::json;

use crate::web::{middleware::auth::AuthenticatedUser, state::AppState};

/// API: returns insight + concepts for a work at GET /api/works/{id}/insight
pub async fn get_work_insight(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Response {
    match state.insight_repo.get_for_work(&id).await {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "message": "No insight available for this work" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal_error", "message": e.to_string() })),
        )
            .into_response(),
    }
}

/// UI: serves the book detail page at GET /works/{id}
pub async fn get_work_detail_page(
    State(_state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Response {
    Html(render_detail_page(&id)).into_response()
}

fn render_detail_page(work_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Book Detail</title>
  <style>
    *, *::before, *::after {{ box-sizing: border-box; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Inter, Arial, sans-serif;
      background: #f5f5f5;
      color: #262626;
      margin: 0;
    }}
    header {{
      background: #1a1a1a;
      color: #f0f0f0;
      padding: 0 2rem;
      height: 56px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }}
    header h1 {{
      font-size: 1rem;
      font-weight: 700;
      margin: 0;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: #f0f0f0;
    }}
    header a {{
      color: #8c8c8c;
      text-decoration: none;
      font-size: 0.875rem;
    }}
    header a:hover {{ color: #f0f0f0; }}
    main {{
      max-width: 860px;
      margin: 2.5rem auto;
      padding: 0 1.5rem;
    }}
    .meta-title {{
      font-size: 1.75rem;
      font-weight: 700;
      margin: 0 0 0.35rem;
      line-height: 1.2;
    }}
    .meta-author {{
      font-size: 1rem;
      color: #595959;
      margin: 0 0 0.5rem;
    }}
    .meta-isbn {{
      font-size: 0.8rem;
      color: #8c8c8c;
      margin: 0 0 1.5rem;
    }}
    .status-badge {{
      display: inline-block;
      padding: 0.25rem 0.65rem;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.04em;
      margin-bottom: 1.5rem;
    }}
    .status-pending {{ background: #e5e5e5; color: #595959; }}
    .status-processing {{ background: #cce0f5; color: #0653b6; }}
    .status-done {{ background: #d4edda; color: #155724; }}
    .status-failed {{ background: #f8d7da; color: #721c24; }}
    .section {{
      margin-bottom: 2rem;
    }}
    .section-title {{
      font-size: 0.75rem;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #8c8c8c;
      margin: 0 0 0.75rem;
    }}
    .insight-box {{
      background: #f9f9f9;
      border-left: 4px solid #1c69d4;
      padding: 1.5rem;
    }}
    .insight-summary {{
      font-size: 0.975rem;
      line-height: 1.65;
      margin: 0;
      color: #262626;
    }}
    .key-points {{
      list-style: none;
      padding: 0;
      margin: 0;
    }}
    .key-points li {{
      position: relative;
      padding: 0.5rem 0 0.5rem 1.25rem;
      font-size: 0.925rem;
      line-height: 1.55;
      border-bottom: 1px solid #f0f0f0;
    }}
    .key-points li:last-child {{ border-bottom: none; }}
    .key-points li::before {{
      content: '';
      position: absolute;
      left: 0;
      top: 0.9rem;
      width: 6px;
      height: 6px;
      background: #1c69d4;
    }}
    .concept-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 0.75rem;
    }}
    .concept-card {{
      background: #fff;
      border: 1px solid #e5e5e5;
      padding: 0.85rem 1rem;
    }}
    .concept-name {{
      font-size: 0.9rem;
      font-weight: 700;
      margin: 0 0 0.25rem;
    }}
    .concept-domain {{
      font-size: 0.72rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.04em;
      margin: 0 0 0.6rem;
    }}
    .relevance-bar-track {{
      height: 4px;
      background: #e5e5e5;
      position: relative;
    }}
    .relevance-bar-fill {{
      height: 100%;
      background: #1c69d4;
    }}
    .relevance-label {{
      font-size: 0.68rem;
      color: #8c8c8c;
      margin-top: 0.3rem;
    }}
    .skeleton-line {{
      height: 1rem;
      background: linear-gradient(90deg, #e5e5e5 25%, #f0f0f0 50%, #e5e5e5 75%);
      background-size: 200% 100%;
      animation: shimmer 1.4s infinite;
      margin-bottom: 0.5rem;
    }}
    @keyframes shimmer {{ to {{ background-position: -200% 0; }} }}
    .skeleton-wide {{ width: 70%; }}
    .skeleton-full {{ width: 100%; }}
    .skeleton-narrow {{ width: 40%; }}
    .error-box {{
      background: #f8d7da;
      border: 1px solid #f5c6cb;
      padding: 1rem 1.25rem;
      color: #721c24;
      font-size: 0.9rem;
    }}
    .processing-notice {{
      color: #595959;
      font-size: 0.9rem;
      padding: 1rem 0;
    }}
    .retry-btn {{
      margin-top: 1rem;
      padding: 0.5rem 1.25rem;
      background: #1c69d4;
      color: #fff;
      border: none;
      font-size: 0.875rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
    }}
    .retry-btn:hover {{ background: #0653b6; }}
    .retry-btn:disabled {{ background: #8c8c8c; cursor: not-allowed; }}
    .openlib-attribution {{
      font-size: 0.75rem;
      color: #8c8c8c;
      margin-top: 2.5rem;
      padding-top: 1rem;
      border-top: 1px solid #e5e5e5;
    }}
    .openlib-attribution a {{
      color: #1c69d4;
      text-decoration: none;
    }}
    .openlib-attribution a:hover {{ text-decoration: underline; }}
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <a href="/">&#8592; Library</a>
  </header>
  <main id="app">
    <!-- Skeleton loader while fetching -->
    <div id="skeleton">
      <div class="skeleton-line skeleton-wide" style="height:1.75rem;margin-bottom:0.5rem;"></div>
      <div class="skeleton-line skeleton-narrow" style="margin-bottom:1rem;"></div>
      <div class="skeleton-line skeleton-full"></div>
      <div class="skeleton-line skeleton-full"></div>
      <div class="skeleton-line skeleton-wide"></div>
    </div>
    <div id="content" style="display:none;"></div>
  </main>

  <script>
    var WORK_ID = {work_id_json};

    function statusClass(status) {{
      return 'status-' + status;
    }}

    function renderDomainColor(domain) {{
      var colors = {{
        'philosophy': '#7c3aed',
        'science': '#0891b2',
        'technology': '#0653b6',
        'history': '#92400e',
        'economics': '#065f46',
        'psychology': '#9d174d',
        'literature': '#1e40af',
        'art': '#b45309',
      }};
      var key = (domain || '').toLowerCase();
      for (var k in colors) {{
        if (key.indexOf(k) !== -1) return colors[k];
      }}
      return '#595959';
    }}

    function esc(s) {{
      return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }}

    function renderWork(work, insight) {{
      var isbnHtml = work.isbn ? '<p class="meta-isbn">ISBN: ' + esc(work.isbn) + '</p>' : '';
      var titleText = work.title || 'Untitled';
      var authorText = work.author ? 'by ' + esc(work.author) : '';

      var statusBadge = '<span class="status-badge ' + statusClass(work.status) + '">' + esc(work.status) + '</span>';

      var insightHtml = '';
      if (work.status === 'done' && insight) {{
        // Summary section
        insightHtml += '<div class="section">';
        insightHtml += '<p class="section-title">Summary</p>';
        insightHtml += '<div class="insight-box"><p class="insight-summary">' + esc(insight.summary) + '</p></div>';
        insightHtml += '</div>';

        // Key points
        if (insight.key_points && insight.key_points.length > 0) {{
          insightHtml += '<div class="section">';
          insightHtml += '<p class="section-title">Key Points</p>';
          insightHtml += '<ul class="key-points">';
          insight.key_points.forEach(function(pt) {{
            insightHtml += '<li>' + esc(pt) + '</li>';
          }});
          insightHtml += '</ul></div>';
        }}

        // Concepts
        if (insight.concepts && insight.concepts.length > 0) {{
          var sorted = insight.concepts.slice().sort(function(a, b) {{ return b.relevance_weight - a.relevance_weight; }});
          insightHtml += '<div class="section">';
          insightHtml += '<p class="section-title">Extracted Concepts (' + sorted.length + ')</p>';
          insightHtml += '<div class="concept-grid">';
          sorted.forEach(function(c) {{
            var pct = Math.round(c.relevance_weight * 100);
            var domColor = renderDomainColor(c.domain);
            insightHtml += '<div class="concept-card">';
            insightHtml += '<p class="concept-name">' + esc(c.display_name) + '</p>';
            insightHtml += '<p class="concept-domain" style="color:' + domColor + '">' + esc(c.domain) + '</p>';
            insightHtml += '<div class="relevance-bar-track"><div class="relevance-bar-fill" style="width:' + pct + '%"></div></div>';
            insightHtml += '<p class="relevance-label">Relevance: ' + (c.relevance_weight).toFixed(2) + '</p>';
            insightHtml += '</div>';
          }});
          insightHtml += '</div></div>';
        }}
      }} else if (work.status === 'processing') {{
        insightHtml = '<p class="processing-notice">&#9203; Processing — extracting insights from this book. This page will update when ready.</p>';
      }} else if (work.status === 'pending') {{
        insightHtml = '<p class="processing-notice">&#9203; In queue — insight extraction has not started yet.</p>';
      }} else if (work.status === 'failed') {{
        var errMsg = work.error_msg ? esc(work.error_msg) : 'An error occurred during processing.';
        insightHtml = '<div class="error-box"><strong>Processing failed:</strong> ' + errMsg + '</div>';
        insightHtml += '<button class="retry-btn" id="retry-btn" onclick="retryWork()">Retry</button>';
      }}

      var olLink = work.open_library_id
        ? '<a href="https://openlibrary.org/works/' + esc(work.open_library_id) + '" target="_blank" rel="noopener">Open Library</a>'
        : '<a href="https://openlibrary.org" target="_blank" rel="noopener">Open Library</a>';

      return '<h2 class="meta-title">' + esc(titleText) + '</h2>'
        + (authorText ? '<p class="meta-author">' + authorText + '</p>' : '')
        + isbnHtml
        + statusBadge
        + insightHtml
        + '<p class="openlib-attribution">Book metadata sourced from ' + olLink + ' (CC BY 4.0).</p>';
    }}

    function retryWork() {{
      var btn = document.getElementById('retry-btn');
      if (btn) btn.disabled = true;
      fetch('/api/works/' + WORK_ID + '/retry', {{
        method: 'POST',
        credentials: 'same-origin'
      }}).then(function(r) {{
        if (r.ok) {{ window.location.reload(); }}
        else {{ if (btn) btn.disabled = false; }}
      }}).catch(function() {{
        if (btn) btn.disabled = false;
      }});
    }}

    function showError(msg) {{
      document.getElementById('skeleton').style.display = 'none';
      var content = document.getElementById('content');
      content.innerHTML = '<div class="error-box">' + msg + '</div><p><a href="/">Back to Library</a></p>';
      content.style.display = 'block';
    }}

    function load() {{
      var workPromise = fetch('/api/works/' + WORK_ID, {{ credentials: 'same-origin' }})
        .then(function(r) {{ return r.json().then(function(d) {{ return {{ ok: r.ok, status: r.status, data: d }}; }}); }});

      workPromise.then(function(r) {{
        if (!r.ok) {{
          if (r.status === 404) {{ showError('Book not found.'); }}
          else {{ showError('Failed to load book data.'); }}
          return;
        }}
        var work = r.data;

        if (work.status !== 'done') {{
          document.getElementById('skeleton').style.display = 'none';
          var content = document.getElementById('content');
          content.innerHTML = renderWork(work, null);
          content.style.display = 'block';
          return;
        }}

        fetch('/api/works/' + WORK_ID + '/insight', {{ credentials: 'same-origin' }})
          .then(function(ir) {{ return ir.json().then(function(d) {{ return {{ ok: ir.ok, data: d }}; }}); }})
          .then(function(ir) {{
            document.getElementById('skeleton').style.display = 'none';
            var content = document.getElementById('content');
            content.innerHTML = renderWork(work, ir.ok ? ir.data : null);
            content.style.display = 'block';
          }})
          .catch(function() {{
            document.getElementById('skeleton').style.display = 'none';
            var content = document.getElementById('content');
            content.innerHTML = renderWork(work, null);
            content.style.display = 'block';
          }});
      }}).catch(function() {{
        showError('Network error. Please check your connection and try again.');
      }});
    }}

    load();
  </script>
</body>
</html>"#,
        work_id_json = serde_json::to_string(work_id).unwrap_or_else(|_| "\"\"".to_string()),
    )
}
