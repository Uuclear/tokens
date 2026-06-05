(function () {
  const UI = window.TOKENS_UI || {};
  const REFRESH_MS = UI.refreshMs || 60000;
  const SCAN_MS = UI.scanMs || 300000;

  let since = 'all';
  let dailyChart = null;
  let hasLoaded = false;
  let inflight = false;

  const wrap = document.getElementById('wrap');
  const grid = document.getElementById('grid');
  const grand = document.getElementById('grand');
  const summary = document.getElementById('summary');
  const chartPanel = document.getElementById('chart-panel');
  const btnRefresh = document.getElementById('refresh');

  document.querySelectorAll('#since-pills .pill').forEach(btn => {
    btn.onclick = () => {
      since = btn.dataset.since;
      document.querySelectorAll('#since-pills .pill').forEach(b => b.classList.toggle('active', b === btn));
      document.getElementById('since-label').textContent = since === 'all' ? '全部' : since;
      load(false);
    };
  });
  btnRefresh.onclick = () => load(false);

  function chartColors() {
    return {
      primary: UI.chartPrimary || '#5b9cff',
      fill: UI.chartFill !== undefined ? UI.chartFill : 'rgba(91,156,255,0.12)',
      tension: UI.chartTension !== undefined ? UI.chartTension : 0.35,
      stepped: !!UI.chartStepped,
      legend: UI.legendColor || '#8b9cb8',
      tick: UI.tickColor || '#5c6d88',
      gridX: UI.gridX || 'rgba(255,255,255,0.04)',
      gridY: UI.gridY || 'rgba(255,255,255,0.06)',
      fillSeries: UI.fillSeries !== false,
    };
  }

  function buildChartDatasets(chart) {
    const c = chartColors();
    const datasets = [{
      label: '合计',
      data: chart.total,
      borderColor: c.primary,
      backgroundColor: c.fill,
      fill: c.fill !== 'transparent' && c.fill !== false,
      tension: c.stepped ? 0 : c.tension,
      stepped: c.stepped,
      pointRadius: UI.pointRadius !== undefined ? UI.pointRadius : 3,
      pointHoverRadius: 5,
      borderWidth: UI.borderWidth !== undefined ? UI.borderWidth : 2.5,
    }];
    (chart.series || []).forEach(s => {
      datasets.push({
        label: s.display_name,
        data: s.values,
        borderColor: s.color,
        backgroundColor: c.fillSeries ? s.color + '18' : 'transparent',
        fill: false,
        tension: c.stepped ? 0 : c.tension,
        stepped: c.stepped,
        pointRadius: 2,
        borderWidth: 1.5,
        borderDash: c.stepped ? [] : [4, 3],
      });
    });
    return datasets;
  }

  function renderDailyChart(chart, smooth) {
    if (!chart || !chart.labels || chart.labels.length === 0) {
      if (!smooth) {
        chartPanel.style.display = 'none';
        if (dailyChart) { dailyChart.destroy(); dailyChart = null; }
      }
      return;
    }
    chartPanel.style.display = 'block';
    const datasets = buildChartDatasets(chart);
    const c = chartColors();
    if (dailyChart) {
      dailyChart.data.labels = chart.labels;
      dailyChart.data.datasets = datasets;
      dailyChart.update(smooth ? 'active' : 'none');
      return;
    }
    const ctx = document.getElementById('daily-chart').getContext('2d');
    dailyChart = new Chart(ctx, {
      type: 'line',
      data: { labels: chart.labels, datasets },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: { duration: smooth ? (UI.animMs || 480) : 0 },
        transitions: { active: { animation: { duration: UI.animMs || 480 } } },
        interaction: { mode: 'index', intersect: false },
        plugins: {
          legend: {
            labels: { color: c.legend, boxWidth: 12, font: { size: 11, family: UI.chartFont } }
          },
          tooltip: {
            callbacks: {
              label: (ctx) => ctx.dataset.label + ': ' + formatNum(ctx.parsed.y)
            }
          }
        },
        scales: {
          x: {
            ticks: { color: c.tick, maxRotation: 45, font: { size: 10, family: UI.chartFont } },
            grid: { color: c.gridX }
          },
          y: {
            ticks: { color: c.tick, callback: (v) => formatNum(v), font: { family: UI.chartFont } },
            grid: { color: c.gridY }
          }
        }
      }
    });
  }

  function applyGrid(platforms, smooth) {
    if (!platforms.length) {
      const hint = UI.emptyHint || 'tokens setup --init 或 tokens scan';
      grid.innerHTML = '<div class="empty"><p style="font-size:1.1rem;margin-bottom:0.5rem">暂无数据</p><p>运行 <code>' + esc(hint) + '</code></p></div>';
      return;
    }
    const html = platforms.map((p, i) => cardHtml(p, i, !smooth)).join('');
    if (!smooth || !grid.querySelector('.card')) {
      grid.innerHTML = html;
      return;
    }
    const prevH = grid.offsetHeight;
    grid.style.minHeight = prevH + 'px';
    grid.style.opacity = '0.72';
    requestAnimationFrame(() => {
      grid.innerHTML = html;
      grid.style.opacity = '1';
      requestAnimationFrame(() => {
        grid.style.minHeight = '';
        grid.querySelectorAll('.card').forEach(el => {
          el.classList.add('card--flash');
          setTimeout(() => el.classList.remove('card--flash'), 520);
        });
      });
    });
  }

  function applyDashboard(data, smooth) {
    grand.textContent = data.grand_total_fmt;
    summary.style.display = 'grid';
    document.getElementById('stat-platforms').textContent = data.platform_count;
    document.getElementById('stat-sessions').textContent = fmtInt(data.total_sessions);
    document.getElementById('stat-calls').textContent = fmtInt(data.total_calls);
    document.getElementById('stat-in').textContent = data.total_input_fmt || '—';
    document.getElementById('stat-out').textContent = data.total_output_fmt || '—';
    renderDailyChart(data.daily_chart, smooth);
    if (!data.platforms.length && !smooth) {
      chartPanel.style.display = 'none';
    }
    applyGrid(data.platforms, smooth);
  }

  async function load(forceLoading) {
    if (inflight) return;
    inflight = true;
    const first = !hasLoaded;
    const smooth = hasLoaded && !forceLoading;

    if (first) {
      grid.innerHTML = '<div class="loading"><div class="spinner"></div>加载中…</div>';
    } else {
      wrap.classList.add('is-refreshing');
      btnRefresh.classList.add('is-busy');
    }

    try {
      const r = await fetch('/api/dashboard?since=' + encodeURIComponent(since));
      if (!r.ok) throw new Error(await r.text());
      const data = await r.json();
      applyDashboard(data, smooth);
      hasLoaded = true;
    } catch (e) {
      if (!hasLoaded) {
        chartPanel.style.display = 'none';
        grid.innerHTML = '<div class="empty">加载失败：' + esc(e.message) + '</div>';
      }
    } finally {
      wrap.classList.remove('is-refreshing');
      btnRefresh.classList.remove('is-busy');
      inflight = false;
    }
  }

  function logoImg(p) {
    if (UI.hideLogos) {
      const ch = (p.display_name || p.id || '?').charAt(0).toUpperCase();
      return '<div class="plat-badge" style="border-color:' + esc(p.color) + ';color:' + esc(p.color) + '">' + esc(ch) + '</div>';
    }
    const src = esc(p.logo_url || '');
    const cls = UI.logoClass ? ' plat-logo ' + UI.logoClass : ' plat-logo';
    return '<img class="' + cls.trim() + '" src="' + src + '" alt="" loading="lazy" />';
  }

  function cardHtml(p, idx, animateEntry) {
    const delay = animateEntry ? Math.min(idx * 0.06, 0.4) : 0;
    const cardClass = animateEntry ? 'card' : 'card card--static';
    const surfaces = (p.surfaces || []).map(s => `
        <div class="surface-row">
          <span class="name">${esc(s.surface)}</span>
          <div class="mini-bar"><i style="width:${s.share_pct.toFixed(1)}%;background:${p.color}"></i></div>
          <div class="vals"><strong>${esc(s.total_tokens)}</strong><span>${s.calls} calls</span></div>
        </div>
      `).join('');
    const models = (p.top_models || []).map(m =>
      '<span class="model-tag"><b>' + esc(shortModel(m.model)) + '</b> ' + esc(m.total_tokens) + '</span>'
    ).join('');
    return `
        <article class="${cardClass}" data-platform="${esc(p.id)}" style="animation-delay:${delay}s">
          <div class="card-head">
            ${logoImg(p)}
            <div>
              <h2>${esc(p.display_name)}</h2>
              <div class="sub">${p.sessions} sessions · ${fmtInt(p.calls)} calls · ${p.active_days} 活跃日</div>
            </div>
            <div class="card-total">
              <div class="num">${esc(p.total_tokens)}</div>
              <div class="pct">${p.share_pct.toFixed(1)}%</div>
            </div>
          </div>
          <div class="share-bar"><i style="width:${p.share_pct}%;background:${p.color}"></i></div>
          <div class="metrics">
            <div class="metric"><div class="k">Input</div><div class="v">${esc(p.input_tokens)}</div></div>
            <div class="metric"><div class="k">Output</div><div class="v">${esc(p.output_tokens)}</div></div>
            <div class="metric"><div class="k">时长</div><div class="v">${esc(p.duration)}</div></div>
          </div>
          <div class="fav">常用模型 <em>${esc(p.favorite_model)}</em></div>
          ${p.surfaces.length ? '<div class="section-title">Surfaces</div><div class="surface-list">' + surfaces + '</div>' : ''}
          ${p.top_models.length ? '<div class="section-title">Top models</div><div class="models">' + models + '</div>' : ''}
        </article>
      `;
  }

  function shortModel(m) {
    if (m.length <= 24) return m;
    return m.slice(0, 22) + '…';
  }
  function fmtInt(n) {
    return n >= 1000 ? (n / 1000).toFixed(1) + 'k' : String(n);
  }
  function formatNum(n) {
    if (n >= 1e9) return (n / 1e9).toFixed(2) + 'b';
    if (n >= 1e6) return (n / 1e6).toFixed(2) + 'm';
    if (n >= 1e3) return (n / 1e3).toFixed(2) + 'k';
    return String(Math.round(n));
  }
  function esc(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  }

  const hintEl = document.getElementById('refresh-hint');
  if (hintEl) {
    hintEl.textContent = UI.refreshHint ||
      ('页面 ' + (REFRESH_MS / 1000) + ' 秒刷新 · 后台 ' + (SCAN_MS / 60000) + ' 分钟扫描');
  }

  load(false);
  setInterval(() => load(false), REFRESH_MS);
})();
