/* =========================================================================
   ABRAXAS — shared sidebar navigation
   Included by every page that renders the shell sidebar. Makes the
   sidebar actually navigate:
     · "+ nova conversa"  → Abraxas New Conversation.html
     · conversation items → Abraxas Chat.html (except on the chat itself,
                            where they switch threads in-page)
     · footer model row   → Abraxas Model Manager.html
   Also injects two quiet links (preferências / falhas) into the footer
   and reflects the last chosen model (localStorage 'abraxas:model').
   ========================================================================= */
(function () {
  const page = document.body.dataset.page || '';
  const go = href => { window.location.href = href; };

  // + nova conversa
  document.querySelectorAll('.side .new-btn').forEach(btn => {
    if (page === 'new') return; // already here
    btn.addEventListener('click', () => go('Abraxas New Conversation.html'));
  });

  // conversation items → open in the chat (chat handles its own switching)
  if (page !== 'chat') {
    document.querySelectorAll('.side .conv').forEach(c => {
      c.style.cursor = 'pointer';
      c.addEventListener('click', () => {
        try { sessionStorage.setItem('abraxas:conv', c.dataset.title || c.textContent.trim()); } catch (e) {}
        go('Abraxas Chat.html');
      });
    });
  }

  const footer = document.querySelector('.side .footer');
  if (footer) {
    // reflect the chosen model, if one was picked
    // (skip on the chat page — it manages its own #footerName / #footerMeta
    //  nodes; rebuilding the row here would orphan those references)
    if (page !== 'chat') {
      try {
        const m = JSON.parse(localStorage.getItem('abraxas:model') || 'null');
        if (m && m.name) {
          const row = footer.querySelector('.model-row');
          const meta = footer.querySelector('.model-meta');
          if (row) {
            const pulse = row.querySelector('.pulse');
            row.textContent = '';
            if (pulse) row.appendChild(pulse);
            row.appendChild(document.createTextNode((m.name + ' · ' + (m.id || '')).toLowerCase()));
          }
          if (meta && m.foot) meta.textContent = m.foot;
        }
      } catch (e) {}
    }

    // model row → ateliê dos modelos
    const row = footer.querySelector('.model-row');
    if (row && page !== 'manager') {
      row.style.cursor = 'pointer';
      row.title = 'ateliê dos modelos';
      row.addEventListener('click', () => go('Abraxas Model Manager.html'));
    }

    // quiet footer links
    const nav = document.createElement('nav');
    nav.setAttribute('aria-label', 'Atalhos');
    nav.style.cssText = 'display:flex;gap:12px;margin-top:10px;padding-top:10px;border-top:1px solid var(--line);';
    const mk = (label, href, here) => {
      const a = document.createElement('a');
      a.textContent = label;
      a.href = href;
      a.style.cssText =
        'font-family:var(--mono);font-size:9px;letter-spacing:0.18em;text-transform:uppercase;' +
        'text-decoration:none;transition:color .15s;color:' + (here ? 'var(--brass)' : 'var(--mute-2)');
      a.onmouseenter = () => { a.style.color = 'var(--brass)'; };
      a.onmouseleave = () => { a.style.color = here ? 'var(--brass)' : 'var(--mute-2)'; };
      return a;
    };
    nav.appendChild(mk('modelos', 'Abraxas Model Manager.html', page === 'manager'));
    nav.appendChild(mk('preferências', 'Abraxas Settings.html', page === 'settings'));
    nav.appendChild(mk('falhas', 'Abraxas Error States.html', page === 'errors'));
    footer.appendChild(nav);
  }
})();
