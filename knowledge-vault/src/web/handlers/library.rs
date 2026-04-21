use axum::{
    extract::{FromRequestParts, State},
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::web::{middleware::auth::AuthenticatedUser, state::AppState};

pub async fn get_library(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    let (mut parts, _body) = req.into_parts();
    match AuthenticatedUser::from_request_parts(&mut parts, &state).await {
        Ok(_auth) => Html(LIBRARY_HTML).into_response(),
        Err(_) => match state.user_repo.count().await {
            Ok(0) => Redirect::to("/setup").into_response(),
            _ => Redirect::to("/login").into_response(),
        },
    }
}

const LIBRARY_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Library</title>
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
    .add-btn {
      display: inline-flex;
      align-items: center;
      gap: 0.4rem;
      padding: 0.45rem 1rem;
      background: #1c69d4;
      color: #fff;
      border: none;
      font-size: 0.875rem;
      font-weight: 600;
      cursor: pointer;
      text-decoration: none;
      font-family: inherit;
    }
    .add-btn:hover { background: #0653b6; }
    .add-btn:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    main {
      max-width: 900px;
      margin: 2.5rem auto;
      padding: 0 1.5rem;
    }
    .page-title {
      font-size: 1.75rem;
      font-weight: 700;
      margin: 0 0 1.5rem;
    }
    .book-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 1rem;
    }
    .book-card {
      background: #fff;
      border: 1px solid #e5e5e5;
      padding: 1rem;
      cursor: pointer;
      text-decoration: none;
      color: inherit;
      display: block;
      transition: background 0.1s;
    }
    .book-card:hover { background: #f5f5f5; }
    .book-card:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .book-title {
      font-size: 0.95rem;
      font-weight: 700;
      margin: 0 0 0.25rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .book-author {
      font-size: 0.8rem;
      color: #595959;
      margin: 0 0 0.75rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .book-meta {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
    }
    .badge {
      display: inline-block;
      padding: 0.2rem 0.5rem;
      font-size: 0.7rem;
      font-weight: 700;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      color: #fff;
    }
    .badge-pending  { background: #ffc107; color: #262626; }
    .badge-processing { background: #1c69d4; animation: pulse 1.5s ease-in-out infinite; }
    .badge-done     { background: #28a745; }
    .badge-failed   { background: #dc3545; }
    @keyframes pulse {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.65; }
    }
    .retry-btn {
      padding: 0.2rem 0.6rem;
      background: transparent;
      color: #dc3545;
      border: 1px solid #dc3545;
      font-size: 0.75rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
    }
    .retry-btn:hover { background: #dc3545; color: #fff; }
    .retry-btn:focus {
      outline: 3px solid #dc3545;
      outline-offset: 2px;
    }
    .empty-state {
      text-align: center;
      padding: 4rem 1rem;
      color: #595959;
    }
    .empty-state p { font-size: 1rem; margin: 0 0 1.5rem; }
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
    .isbn-label {
      font-size: 0.7rem;
      color: #8c8c8c;
    }
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <a href="/add" class="add-btn" aria-label="Add a book to your vault">+ Add Book</a>
  </header>
  <main>
    <h2 class="page-title">Library</h2>
    <div id="content">
      <div class="loading-state" aria-live="polite" aria-label="Loading books">Loading…</div>
    </div>
  </main>

  <script>
    var content = document.getElementById('content');

    function statusBadgeHtml(status) {
      var cls = 'badge badge-' + status;
      var label = status.charAt(0).toUpperCase() + status.slice(1);
      return '<span class="' + cls + '">' + label + '</span>';
    }

    function escapeHtml(str) {
      if (!str) return '';
      return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }

    function renderBooks(works) {
      if (!works || works.length === 0) {
        content.innerHTML =
          '<div class="empty-state">' +
            '<p>No books yet. Add your first book to get started.</p>' +
            '<a href="/add" class="add-btn">+ Add Book</a>' +
          '</div>';
        return;
      }

      var html = '<div class="book-grid" id="book-grid">';
      works.forEach(function(w) {
        var title = escapeHtml(w.title) || '(untitled)';
        var author = escapeHtml(w.author) || '';
        var isbn = w.isbn ? escapeHtml(w.isbn) : '';
        var retryBtn = w.status === 'failed'
          ? '<button class="retry-btn" data-id="' + escapeHtml(w.id) + '" aria-label="Retry processing for ' + title + '">Retry</button>'
          : '';
        var metaLeft = statusBadgeHtml(w.status);
        var metaRight = retryBtn || (isbn ? '<span class="isbn-label">' + isbn + '</span>' : '');

        html +=
          '<a href="/works/' + escapeHtml(w.id) + '" class="book-card" data-id="' + escapeHtml(w.id) + '">' +
            '<p class="book-title">' + title + '</p>' +
            '<p class="book-author">' + (author || '&nbsp;') + '</p>' +
            '<div class="book-meta">' +
              '<span>' + metaLeft + '</span>' +
              '<span>' + metaRight + '</span>' +
            '</div>' +
          '</a>';
      });
      html += '</div>';
      content.innerHTML = html;

      // Retry buttons: stop event propagation so card click doesn't fire
      content.querySelectorAll('.retry-btn').forEach(function(btn) {
        btn.addEventListener('click', function(e) {
          e.preventDefault();
          e.stopPropagation();
          retryWork(btn.dataset.id, btn);
        });
      });
    }

    function retryWork(id, btn) {
      btn.disabled = true;
      btn.textContent = 'Retrying…';
      fetch('/api/works/' + id + '/retry', {
        method: 'POST',
        credentials: 'same-origin',
      })
      .then(function(res) {
        if (res.ok) {
          loadBooks();
        } else {
          btn.disabled = false;
          btn.textContent = 'Retry';
        }
      })
      .catch(function() {
        btn.disabled = false;
        btn.textContent = 'Retry';
      });
    }

    function updateCard(work) {
      var card = document.querySelector('.book-card[data-id="' + work.id + '"]');
      if (!card) return;

      var badgeEl = card.querySelector('.badge');
      if (badgeEl) {
        badgeEl.className = 'badge badge-' + work.status;
        badgeEl.textContent = work.status.charAt(0).toUpperCase() + work.status.slice(1);
      }

      var retryEl = card.querySelector('.retry-btn');
      if (work.status === 'failed' && !retryEl) {
        var meta = card.querySelector('.book-meta span:last-child');
        if (meta) {
          var btn = document.createElement('button');
          btn.className = 'retry-btn';
          btn.dataset.id = work.id;
          btn.textContent = 'Retry';
          btn.addEventListener('click', function(e) {
            e.preventDefault();
            e.stopPropagation();
            retryWork(work.id, btn);
          });
          meta.innerHTML = '';
          meta.appendChild(btn);
        }
      } else if (work.status !== 'failed' && retryEl) {
        retryEl.remove();
      }

      if (work.title) {
        var titleEl = card.querySelector('.book-title');
        if (titleEl) titleEl.textContent = work.title;
      }
      if (work.author) {
        var authorEl = card.querySelector('.book-author');
        if (authorEl) authorEl.textContent = work.author;
      }
    }

    function loadBooks() {
      fetch('/api/works?limit=200', { credentials: 'same-origin' })
        .then(function(res) {
          if (!res.ok) throw new Error('HTTP ' + res.status);
          return res.json();
        })
        .then(function(works) {
          renderBooks(works);
          connectWs();
        })
        .catch(function(err) {
          content.innerHTML =
            '<div class="error-state" role="alert">Failed to load library: ' + escapeHtml(err.message) + '. <a href="/">Retry</a></div>';
        });
    }

    var wsConnected = false;
    function connectWs() {
      if (wsConnected) return;
      try {
        var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        var ws = new WebSocket(proto + '//' + location.host + '/ws');
        ws.onopen = function() { wsConnected = true; };
        ws.onmessage = function(ev) {
          try {
            var msg = JSON.parse(ev.data);
            if (msg && msg.type === 'work_status_updated' && msg.work) {
              updateCard(msg.work);
            }
          } catch (_) {}
        };
        ws.onclose = function() {
          wsConnected = false;
          setTimeout(connectWs, 3000);
        };
      } catch (_) {}
    }

    loadBooks();
  </script>
</body>
</html>"#;
