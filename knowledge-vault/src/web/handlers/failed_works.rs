use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::web::{middleware::auth::AuthenticatedUser, state::AppState};

pub async fn get_failed_works(
    State(state): State<AppState>,
    auth: Result<AuthenticatedUser, Response>,
) -> Response {
    match auth {
        Ok(_) => Html(FAILED_WORKS_HTML).into_response(),
        Err(_) => match state.user_repo.count().await {
            Ok(0) => Redirect::to("/setup").into_response(),
            _ => Redirect::to("/login").into_response(),
        },
    }
}

const FAILED_WORKS_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Failed Works</title>
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
      max-width: 900px;
      margin: 2.5rem auto;
      padding: 0 1.5rem;
    }
    .page-header {
      display: flex;
      align-items: baseline;
      gap: 0.75rem;
      margin: 0 0 1.5rem;
    }
    .page-title {
      font-size: 1.75rem;
      font-weight: 700;
      margin: 0;
    }
    .failure-count {
      display: inline-block;
      background: #dc3545;
      color: #fff;
      font-size: 0.75rem;
      font-weight: 700;
      padding: 0.15rem 0.5rem;
      min-width: 1.5rem;
      text-align: center;
    }
    .failed-list {
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }
    .failed-card {
      background: #fff;
      border: 1px solid #e5e5e5;
      border-left: 4px solid #dc3545;
      padding: 1.25rem 1.25rem 1rem;
    }
    .card-header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 1rem;
      margin-bottom: 0.75rem;
    }
    .card-meta { flex: 1; min-width: 0; }
    .book-title {
      font-size: 1rem;
      font-weight: 700;
      margin: 0 0 0.2rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .book-author {
      font-size: 0.85rem;
      color: #595959;
      margin: 0 0 0.2rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .book-isbn {
      font-size: 0.72rem;
      color: #8c8c8c;
      margin: 0;
    }
    .badge-failed {
      display: inline-block;
      padding: 0.2rem 0.5rem;
      font-size: 0.7rem;
      font-weight: 700;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      background: #dc3545;
      color: #fff;
      white-space: nowrap;
      flex-shrink: 0;
    }
    .error-box {
      background: #f8d7da;
      border: 1px solid #f5c6cb;
      padding: 0.75rem 1rem;
      color: #721c24;
      font-size: 0.875rem;
      line-height: 1.5;
      margin-bottom: 0.75rem;
      word-break: break-word;
    }
    .error-label {
      font-weight: 700;
      margin-right: 0.35rem;
    }
    .card-actions {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
    }
    .detail-link {
      font-size: 0.8rem;
      color: #1c69d4;
      text-decoration: none;
    }
    .detail-link:hover { text-decoration: underline; }
    .detail-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    .retry-btn {
      padding: 0.35rem 0.9rem;
      background: transparent;
      color: #dc3545;
      border: 1px solid #dc3545;
      font-size: 0.8rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
    }
    .retry-btn:hover { background: #dc3545; color: #fff; }
    .retry-btn:focus {
      outline: 3px solid #dc3545;
      outline-offset: 2px;
    }
    .retry-btn:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
    .empty-state {
      text-align: center;
      padding: 4rem 1rem;
      color: #595959;
      background: #fff;
      border: 1px solid #e5e5e5;
    }
    .empty-state p { font-size: 1rem; margin: 0 0 0.5rem; }
    .empty-state .sub { font-size: 0.875rem; color: #8c8c8c; margin: 0 0 1.5rem; }
    .back-link {
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
    .back-link:hover { background: #0653b6; }
    .back-link:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
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
    .retry-success {
      display: inline-block;
      font-size: 0.8rem;
      color: #155724;
      font-weight: 600;
    }
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <nav class="header-nav" aria-label="Site navigation">
      <a href="/" class="nav-link">&#8592; Library</a>
      <a href="/add" class="nav-link">+ Add Book</a>
    </nav>
  </header>
  <main>
    <div class="page-header">
      <h2 class="page-title">Failed Works</h2>
      <span id="failure-count" class="failure-count" aria-live="polite" aria-label="Number of failed works" style="display:none;"></span>
    </div>
    <div id="content" aria-live="polite">
      <div class="loading-state" aria-label="Loading failed works">Loading…</div>
    </div>
  </main>

  <script>
    var content = document.getElementById('content');
    var countBadge = document.getElementById('failure-count');
    var failedWorks = [];

    function escapeHtml(str) {
      if (!str) return '';
      return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }

    function updateCountBadge(n) {
      if (n > 0) {
        countBadge.textContent = n;
        countBadge.style.display = '';
      } else {
        countBadge.style.display = 'none';
      }
    }

    function renderFailedWorks(works) {
      if (!works || works.length === 0) {
        updateCountBadge(0);
        content.innerHTML =
          '<div class="empty-state">' +
            '<p>No failed works</p>' +
            '<p class="sub">All books have been processed successfully.</p>' +
            '<a href="/" class="back-link">Go to Library</a>' +
          '</div>';
        return;
      }

      updateCountBadge(works.length);

      var html = '<ul class="failed-list" role="list" aria-label="Failed books">';
      works.forEach(function(w) {
        var title = escapeHtml(w.title) || '(untitled)';
        var author = escapeHtml(w.author) || '';
        var isbn = w.isbn ? escapeHtml(w.isbn) : '';
        var errMsg = w.error_msg
          ? escapeHtml(w.error_msg)
          : 'An error occurred during processing. No additional details are available.';

        html +=
          '<li class="failed-card" data-id="' + escapeHtml(w.id) + '" role="listitem">' +
            '<div class="card-header">' +
              '<div class="card-meta">' +
                '<p class="book-title">' + title + '</p>' +
                (author ? '<p class="book-author">by ' + author + '</p>' : '') +
                (isbn ? '<p class="book-isbn">ISBN: ' + isbn + '</p>' : '') +
              '</div>' +
              '<span class="badge-failed" aria-label="Status: failed">Failed</span>' +
            '</div>' +
            '<div class="error-box" role="alert" aria-label="Error details">' +
              '<span class="error-label">Error:</span>' + errMsg +
            '</div>' +
            '<div class="card-actions">' +
              '<a href="/works/' + escapeHtml(w.id) + '" class="detail-link" aria-label="View details for ' + title + '">View details</a>' +
              '<button class="retry-btn" data-id="' + escapeHtml(w.id) + '" aria-label="Retry processing for ' + title + '">Retry</button>' +
            '</div>' +
          '</li>';
      });
      html += '</ul>';
      content.innerHTML = html;

      content.querySelectorAll('.retry-btn').forEach(function(btn) {
        btn.addEventListener('click', function() {
          retryWork(btn.dataset.id, btn);
        });
      });
    }

    function findCard(workId) {
      var cards = content.querySelectorAll('.failed-card');
      for (var i = 0; i < cards.length; i++) {
        if (cards[i].dataset.id === workId) return cards[i];
      }
      return null;
    }

    function retryWork(id, btn) {
      btn.disabled = true;
      btn.textContent = 'Retrying…';
      fetch('/api/works/' + encodeURIComponent(id) + '/retry', {
        method: 'POST',
        credentials: 'same-origin',
      })
      .then(function(res) {
        if (res.ok) {
          var card = findCard(id);
          if (card) {
            var actions = card.querySelector('.card-actions');
            if (actions) {
              btn.remove();
              var ok = document.createElement('span');
              ok.className = 'retry-success';
              ok.textContent = '✓ Queued for retry';
              actions.appendChild(ok);
            }
          }
          failedWorks = failedWorks.filter(function(w) { return w.id !== id; });
          updateCountBadge(failedWorks.length);
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

    function removeCard(workId) {
      var card = findCard(workId);
      if (card) card.remove();
      failedWorks = failedWorks.filter(function(w) { return w.id !== workId; });
      updateCountBadge(failedWorks.length);
      var list = document.querySelector('.failed-list');
      if (list && list.children.length === 0) {
        renderFailedWorks([]);
      }
    }

    function fetchAllWorks(callback) {
      var PAGE = 200;
      var offset = 0;
      var all = [];
      function next() {
        fetch('/api/works?limit=' + PAGE + '&offset=' + offset, { credentials: 'same-origin' })
          .then(function(res) {
            if (!res.ok) throw new Error('HTTP ' + res.status);
            return res.json();
          })
          .then(function(page) {
            all = all.concat(page || []);
            if (page && page.length === PAGE) {
              offset += PAGE;
              next();
            } else {
              callback(null, all);
            }
          })
          .catch(function(err) { callback(err); });
      }
      next();
    }

    function loadFailedWorks() {
      fetchAllWorks(function(err, works) {
        if (err) {
          content.innerHTML =
            '<div class="error-state" role="alert">Failed to load works: ' + escapeHtml(err.message) +
            '. <button onclick="loadFailedWorks()" style="margin-left:0.5rem;background:none;border:none;color:#721c24;text-decoration:underline;cursor:pointer;font:inherit;padding:0;">Retry</button></div>';
          return;
        }
        failedWorks = (works || []).filter(function(w) { return w.status === 'failed'; });
        renderFailedWorks(failedWorks);
        connectWs();
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
              if (msg.work.status !== 'failed') {
                removeCard(msg.work.id);
              }
            }
          } catch (_) {}
        };
        ws.onclose = function() {
          wsConnected = false;
          setTimeout(connectWs, 3000);
        };
      } catch (_) {}
    }

    loadFailedWorks();
  </script>
</body>
</html>"#;
