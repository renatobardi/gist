use axum::response::{Html, IntoResponse, Response};

use crate::web::middleware::auth::AuthenticatedUser;

pub async fn get_add_book(_auth: AuthenticatedUser) -> Response {
    Html(ADD_BOOK_HTML).into_response()
}

const ADD_BOOK_HTML: &str = r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Knowledge Vault — Adicionar Livro</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'BMW Type Next', Inter, Arial, sans-serif;
      background: #f5f5f5;
      color: #262626;
      margin: 0;
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
    header a {
      color: #8c8c8c;
      text-decoration: none;
      font-size: 0.875rem;
    }
    header a:hover { color: #f0f0f0; }
    main {
      max-width: 600px;
      margin: 3rem auto;
      padding: 0 1.5rem;
    }
    h2 {
      font-size: 1.5rem;
      font-weight: 700;
      margin: 0 0 0.5rem;
    }
    .subtitle {
      color: #595959;
      font-size: 0.9rem;
      margin: 0 0 2rem;
    }
    .card {
      background: #fff;
      border: 1px solid #e5e5e5;
      padding: 2rem;
    }
    .field {
      margin-bottom: 1.5rem;
    }
    label {
      display: block;
      font-size: 0.875rem;
      font-weight: 600;
      margin-bottom: 0.35rem;
      color: #262626;
    }
    input[type="text"] {
      display: block;
      width: 100%;
      padding: 0.6rem 0.75rem;
      border: 1px solid #cccccc;
      border-radius: 0;
      background: #fff;
      color: #262626;
      font-size: 0.95rem;
      font-family: inherit;
      transition: border-color 0.1s, outline 0.1s;
    }
    input[type="text"]:hover {
      border-color: #1c69d4;
    }
    input[type="text"]:focus {
      outline: 3px solid #0653b6;
      outline-offset: 0;
      border-color: #1c69d4;
    }
    input[type="text"].invalid {
      border-color: #dc3545;
    }
    input[type="text"].invalid:focus {
      outline-color: #dc3545;
    }
    .helper {
      font-size: 0.75rem;
      color: #595959;
      margin-top: 0.35rem;
    }
    .error-msg {
      font-size: 0.75rem;
      color: #dc3545;
      margin-top: 0.35rem;
      display: none;
    }
    .error-msg.visible {
      display: block;
    }
    .actions {
      display: flex;
      gap: 1rem;
      align-items: center;
    }
    button[type="submit"] {
      padding: 0.6rem 1.5rem;
      background: #1c69d4;
      color: #fff;
      border: none;
      font-size: 0.95rem;
      font-weight: 600;
      cursor: pointer;
      font-family: inherit;
    }
    button[type="submit"]:hover { background: #0653b6; }
    button[type="submit"]:focus {
      outline: 3px solid #0653b6;
      outline-offset: 2px;
    }
    button[type="submit"]:disabled {
      background: #8c8c8c;
      cursor: not-allowed;
    }
    .cancel-link {
      color: #595959;
      text-decoration: none;
      font-size: 0.9rem;
    }
    .cancel-link:hover { color: #262626; }
    .alert {
      padding: 0.75rem 1rem;
      margin-bottom: 1.25rem;
      font-size: 0.9rem;
      display: none;
    }
    .alert.visible { display: block; }
    .alert-success {
      background: #d4edda;
      border: 1px solid #28a745;
      color: #155724;
    }
    .alert-error {
      background: #f8d7da;
      border: 1px solid #dc3545;
      color: #721c24;
    }
    .spinner {
      display: none;
      width: 14px;
      height: 14px;
      border: 2px solid rgba(255,255,255,0.4);
      border-top-color: #fff;
      border-radius: 50%;
      animation: spin 0.6s linear infinite;
      vertical-align: middle;
      margin-right: 0.4rem;
    }
    button[type="submit"].loading .spinner { display: inline-block; }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
  <header>
    <h1>Knowledge Vault</h1>
    <a href="/">&#8592; Biblioteca</a>
  </header>
  <main>
    <h2>Adicionar Livro</h2>
    <p class="subtitle">Insira um ISBN-10, ISBN-13 ou título do livro para adicioná-lo ao vault.</p>

    <div class="card">
      <div id="alert-success" class="alert alert-success" role="alert" aria-live="polite"></div>
      <div id="alert-error" class="alert alert-error" role="alert" aria-live="polite"></div>

      <form id="add-form" novalidate>
        <div class="field">
          <label for="identifier">ISBN ou Título</label>
          <input
            type="text"
            id="identifier"
            name="identifier"
            placeholder="ex.: 9780132350884 ou Clean Code"
            autocomplete="off"
            aria-required="true"
            aria-describedby="identifier-helper identifier-error"
          >
          <p class="helper" id="identifier-helper">
            ISBN-10, ISBN-13 (hifens permitidos), ou título livre.
          </p>
          <p class="error-msg" id="identifier-error" role="alert"></p>
        </div>

        <div class="actions">
          <button type="submit" id="submit-btn">
            <span class="spinner" aria-hidden="true"></span>
            Enviar
          </button>
          <a href="/" class="cancel-link">Cancelar</a>
        </div>
      </form>
    </div>
  </main>

  <script>
    // ISBN validation (mirrors server-side logic in domain/work.rs)
    function stripHyphens(s) {
      return s.replace(/-/g, '');
    }

    function validateIsbn13(digits) {
      var sum = 0;
      for (var i = 0; i < 13; i++) {
        var d = parseInt(digits[i], 10);
        sum += (i % 2 === 0) ? d : d * 3;
      }
      return sum % 10 === 0;
    }

    function validateIsbn10(digits) {
      if (!/^[0-9]{9}[0-9X]$/.test(digits)) return false;
      var sum = 0;
      for (var i = 0; i < 9; i++) {
        sum += parseInt(digits[i], 10) * (10 - i);
      }
      var last = digits[9] === 'X' ? 10 : parseInt(digits[9], 10);
      sum += last;
      return sum % 11 === 0;
    }

    // Returns { type: 'isbn'|'title', error: string|null }
    function detectAndValidate(raw) {
      var stripped = stripHyphens(raw.trim());

      // Looks like an ISBN attempt if it contains only digits, hyphens, and maybe trailing X
      var isbnAttempt = /^[0-9\-]{10,17}X?$/.test(raw.trim());

      if (!isbnAttempt) {
        // Treat as title — no client-side validation needed
        return { type: 'title', error: null };
      }

      // Validate as ISBN
      if (stripped.length === 13) {
        if (!/^[0-9]{13}$/.test(stripped)) {
          return { type: 'isbn', error: 'ISBN-13 inválido — deve conter apenas dígitos' };
        }
        if (!validateIsbn13(stripped)) {
          return { type: 'isbn', error: 'ISBN-13 inválido — dígito verificador incorreto' };
        }
        return { type: 'isbn', error: null };
      }

      if (stripped.length === 10) {
        if (!validateIsbn10(stripped)) {
          return { type: 'isbn', error: 'ISBN-10 inválido — dígito verificador incorreto' };
        }
        return { type: 'isbn', error: null };
      }

      return { type: 'isbn', error: 'ISBN inválido — esperado 10 ou 13 dígitos, encontrado ' + stripped.length };
    }

    var input = document.getElementById('identifier');
    var errorEl = document.getElementById('identifier-error');
    var submitBtn = document.getElementById('submit-btn');
    var alertSuccess = document.getElementById('alert-success');
    var alertError = document.getElementById('alert-error');
    var form = document.getElementById('add-form');

    function clearFieldError() {
      input.classList.remove('invalid');
      errorEl.textContent = '';
      errorEl.classList.remove('visible');
      input.removeAttribute('aria-invalid');
    }

    function showFieldError(msg) {
      input.classList.add('invalid');
      errorEl.textContent = msg;
      errorEl.classList.add('visible');
      input.setAttribute('aria-invalid', 'true');
    }

    function showAlert(type, msg) {
      alertSuccess.classList.remove('visible');
      alertError.classList.remove('visible');
      if (type === 'success') {
        alertSuccess.textContent = msg;
        alertSuccess.classList.add('visible');
      } else {
        alertError.textContent = msg;
        alertError.classList.add('visible');
      }
    }

    function setLoading(loading) {
      submitBtn.disabled = loading;
      if (loading) {
        submitBtn.classList.add('loading');
      } else {
        submitBtn.classList.remove('loading');
      }
    }

    // Validate on blur
    input.addEventListener('blur', function() {
      var raw = input.value.trim();
      if (!raw) return;
      var result = detectAndValidate(raw);
      if (result.error) {
        showFieldError(result.error);
      }
    });

    // Clear error while user types
    input.addEventListener('input', function() {
      if (input.classList.contains('invalid')) {
        clearFieldError();
      }
    });

    form.addEventListener('submit', function(e) {
      e.preventDefault();

      var raw = input.value.trim();
      if (!raw) {
        showFieldError('Por favor, insira um ISBN ou título.');
        input.focus();
        return;
      }

      var result = detectAndValidate(raw);
      if (result.error) {
        showFieldError(result.error);
        input.focus();
        return;
      }

      clearFieldError();
      alertSuccess.classList.remove('visible');
      alertError.classList.remove('visible');
      setLoading(true);

      var identifier = result.type === 'isbn' ? stripHyphens(raw) : raw;

      fetch('/api/works', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'same-origin',
        body: JSON.stringify({
          identifier: identifier,
          identifier_type: result.type
        })
      })
      .then(function(res) {
        return res.json().then(function(data) {
          return { ok: res.ok, status: res.status, data: data };
        });
      })
      .then(function(r) {
        setLoading(false);
        if (r.ok) {
          input.value = '';
          showAlert('success', 'Livro enviado — o processamento começará em breve. Redirecionando para a biblioteca…');
          setTimeout(function() { window.location.href = '/'; }, 1500);
        } else if (r.status === 409) {
          showAlert('error', 'Este livro já está no seu vault.');
        } else if (r.status === 422 && r.data && r.data.message) {
          showAlert('error', r.data.message);
        } else {
          showAlert('error', 'Algo deu errado. Tente novamente.');
        }
      })
      .catch(function() {
        setLoading(false);
        showAlert('error', 'Erro de rede. Verifique sua conexão e tente novamente.');
      });
    });
  </script>
</body>
</html>"#;
