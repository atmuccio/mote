(function () {
  'use strict';
  // Every element is built with createElement/textContent: report data never becomes markup.
  var D = JSON.parse(document.getElementById('mote-data').textContent);
  var SEV = ['HIGH', 'MEDIUM', 'LOW'];
  var SEVC = ['high', 'medium', 'low'];
  var STATUS = ['', 'new', 'unchanged', 'worsened'];
  var hasBase = !!D.base;
  var PAGE = 200;

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined && text !== null) e.textContent = String(text);
    return e;
  }
  function add(parent) {
    for (var i = 1; i < arguments.length; i++) {
      var c = arguments[i];
      if (c === undefined || c === null || c === false) continue;
      parent.appendChild(typeof c === 'string' || typeof c === 'number' ? document.createTextNode(String(c)) : c);
    }
    return parent;
  }
  function link(href, text, cls) {
    var a = el('a', cls, text);
    a.href = href;
    return a;
  }
  function fmt(n) {
    return Number(n || 0).toLocaleString('en-US');
  }
  function hours(min) {
    min = Number(min || 0);
    var h = min / 60;
    if (min === 0) return '0 h';
    if (min < 6) return '< 0.1 h';
    if (h < 9.95) return h.toFixed(1) + ' h';
    return fmt(Math.round(h)) + ' h';
  }
  function pct(v) {
    return Number(v || 0).toFixed(1) + '%';
  }
  function sum(a) {
    return a.reduce(function (x, y) { return x + y; }, 0);
  }
  function panel(title) {
    var s = el('section', 'panel');
    if (title) add(s, el('h2', null, title));
    return s;
  }
  function table(head, numeric) {
    var wrap = el('div', 'scroll');
    var t = el('table');
    var tr = el('tr');
    head.forEach(function (h, i) {
      add(tr, el('th', numeric && numeric[i] ? 'n' : null, h));
    });
    add(t, add(el('thead'), tr));
    var body = el('tbody');
    add(t, body);
    add(wrap, t);
    return { wrap: wrap, body: body };
  }
  function row(body, cells, numeric) {
    var tr = el('tr');
    cells.forEach(function (c, i) {
      var td = el('td', numeric && numeric[i] ? 'n' : null);
      if (c instanceof Node) add(td, c); else td.textContent = c === undefined || c === null ? '' : String(c);
      add(tr, td);
    });
    add(body, tr);
    return tr;
  }
  function sevBadge(i) {
    return el('span', 'sev ' + SEVC[i], SEV[i]);
  }
  function statusBadge(s) {
    return s === 1 || s === 3 ? el('span', 'st ' + STATUS[s], STATUS[s]) : null;
  }
  function gradeBadge(g) {
    return el('span', 'grade g' + g, g);
  }
  function card(k, v, d) {
    var c = el('div', 'card');
    add(c, el('div', 'k', k));
    var vv = el('div', 'v');
    add(vv, v);
    add(c, vv, d ? el('div', 'd', d) : null);
    return c;
  }
  function counts(c) {
    var s = el('span', 'counts');
    var any = false;
    for (var i = 0; i < 3; i++) {
      if (!c[i]) continue;
      if (any) add(s, ' ');
      add(s, add(el('b', SEVC[i]), fmt(c[i]) + ' ' + SEV[i][0]));
      any = true;
    }
    if (!any) add(s, 'no findings');
    return s;
  }
  function bar(value, max) {
    var b = el('div', 'bar');
    b.style.width = (max > 0 ? Math.max(1, Math.round((value / max) * 100)) : 0) + '%';
    return b;
  }

  var files = D.files.map(function (f, i) {
    return { i: i, path: f[0], lang: f[1], test: f[2], ncloc: f[3], c: f[4], effort: f[5], change: f[6] };
  });
  var byPath = {};
  files.forEach(function (f) { byPath[f.path] = f; });
  var findings = D.findings.map(function (x) {
    return { file: files[x[0]], rule: D.rules[x[1]], line: x[2], col: x[3], sev: x[4], status: x[5], msg: D.messages[x[6]] };
  });
  var byFile = {};
  var byRule = {};
  findings.forEach(function (f) {
    (byFile[f.file.path] = byFile[f.file.path] || []).push(f);
    (byRule[f.rule.id] = byRule[f.rule.id] || []).push(f);
  });
  var rulesById = {};
  D.rules.forEach(function (r) { rulesById[r.id] = r; });

  // ---------------------------------------------------------------- page frame
  var header = el('header', 'top');
  var title = 'Mote report';
  var sub = [];
  if (D.source.kind) sub.push(D.source.kind + (D.source.rev ? ' ' + D.source.rev : ''));
  if (D.source.commit) sub.push('commit ' + D.source.commit.slice(0, 12));
  if (D.source.prefix) sub.push('scan root ' + D.source.prefix);
  if (hasBase) {
    sub.push(D.base.baseline
      ? 'compared with the baseline ' + D.base.base + (D.base.merge_base ? ' (commit ' + D.base.merge_base + ')' : '')
      : 'compared with ' + D.base.base + ' (merge base ' + D.base.merge_base + ')');
  }
  sub.push('Mote ' + D.version);
  add(header, el('h1', null, title), el('div', 'sub', sub.join(' · ')));
  var nav = el('nav', 'tabs');
  var TABS = [['overview', 'Overview'], ['files', 'Files'], ['findings', 'Findings'], ['rules', 'Rules']];
  var tabLinks = {};
  TABS.forEach(function (t) {
    var a = link('#' + t[0], t[1]);
    add(nav, a);
    tabLinks[t[0]] = a;
  });
  add(header, nav);
  var main = el('main');
  var footer = el('footer', null,
    'Generated by Mote ' + D.version + '. A self-contained page: no network access, no source code. ' +
    'Estimated effort is a relative yardstick, not a schedule.');
  add(document.body, header, main, footer);

  function capNote() {
    if (!D.omitted_findings) return null;
    return el('p', 'note warn',
      'This page lists the ' + fmt(D.findings.length) + ' most severe of ' + fmt(D.total_findings) +
      ' findings (at most ' + fmt(D.max_findings) + '); ' + fmt(D.omitted_findings) +
      ' less severe ones are left out. Counts and measures cover all findings; the JSON or SARIF results list them all.');
  }

  // ---------------------------------------------------------------- overview
  function overview() {
    var g = panel(null);
    var passed = D.gate.passed;
    add(g, el('div', 'banner ' + (passed ? 'pass' : 'fail'), 'Quality gate ' + (passed ? 'passed' : 'failed')));
    if (hasBase && D.base.baseline) {
      add(g, el('p', 'note', 'Compared with the baseline ' + D.base.base +
        (D.base.merge_base ? ' (commit ' + D.base.merge_base + ')' : '') +
        ': findings not in it are new, and the conditions apply to them. There is no line diff, so conditions on new lines are skipped.'));
    } else if (hasBase) {
      var nc = D.measures.new_code;
      add(g, el('p', 'note', 'Compared with ' + D.base.base + ' (merge base ' + D.base.merge_base + '): ' +
        fmt(nc.changed_files) + ' files changed, ' + fmt(nc.new_lines) + ' new lines. Conditions apply to new code.'));
    } else {
      add(g, el('p', 'note', 'No base to compare with: the gate applies to all findings.'));
    }
    if (D.gate.conditions.length) {
      var t = table(['Condition', 'Result', 'Actual', 'Threshold'], [0, 0, 1, 1]);
      D.gate.conditions.forEach(function (c) {
        var res = el('span', 'res ' + c.result, c.result === 'skip' ? 'skipped' : c.result === 'pass' ? 'passed' : 'failed');
        if (c.reason) add(res, ' (' + c.reason + ')');
        row(t.body, [c.label, res, c.actual, c.threshold], [0, 0, 1, 1]);
      });
      add(g, t.wrap);
    }
    add(main, g);

    var m = D.measures.total;
    var cards = el('div', 'cards');
    if (D.grades) {
      var gr = D.grades;
      add(cards, card('Maintainability grade', gradeBadge(gr.maintainability.grade),
        gr.maintainability.value.toFixed(2) + ' estimated h per 1,000 lines'));
      add(cards, card('Reliability grade', gradeBadge(gr.reliability.grade),
        gr.reliability.value.toFixed(2) + ' weighted findings per 10,000 lines'));
      if (gr.new_maintainability) {
        add(cards, card('Maintainability, new code', gradeBadge(gr.new_maintainability.grade),
          gr.new_maintainability.value.toFixed(2) + ' h per 1,000 new lines'));
      }
      if (gr.new_reliability) {
        add(cards, card('Reliability, new code', gradeBadge(gr.new_reliability.grade),
          gr.new_reliability.value.toFixed(2) + ' per 10,000 new lines'));
      }
      if (gr.new_not_rated) {
        var nr = gr.new_not_rated;
        add(cards, card('Grades, new code', el('span', null, 'not rated'),
          'only ' + fmt(nr.new_lines) + ' new line' + (nr.new_lines === 1 ? '' : 's') +
          '; grades need at least ' + fmt(nr.min_new_lines)));
      }
    }
    add(cards, card('Lines of code', fmt(m.ncloc), fmt(D.measures.test.ncloc) + ' in tests · ' + fmt(m.files) + ' files'));
    add(cards, card('Findings', fmt(sum(D.counts.total)),
      (hasBase ? fmt(sum(D.counts.new)) + ' new · ' : '') + fmt(D.counts.suppressed) + ' suppressed'));
    add(cards, card('Estimated effort', hours(m.effort_minutes.total),
      hasBase ? hours(D.measures.new_code.new_effort_minutes.total) + ' on new code' : 'reliability ' +
        hours(m.effort_minutes.reliability) + ' · maintainability ' + hours(m.effort_minutes.maintainability)));
    add(cards, card('Complexity', fmt(m.cognitive_complexity), 'cognitive · ' + fmt(m.cyclomatic_complexity) +
      ' cyclomatic · ' + fmt(m.functions) + ' functions'));
    add(cards, card('Duplication', pct(m.duplicated_lines_density),
      hasBase && !D.base.baseline ? pct(D.measures.new_code.new_duplication_pct) + ' on new code' : fmt(m.duplicated_lines) + ' lines'));
    var cov = D.measures.coverage;
    var ncov = hasBase ? D.measures.new_code.new_coverage_pct : null;
    if (cov || ncov !== null && ncov !== undefined) {
      add(cards, card('Coverage', cov ? pct(cov.coverage_pct) : '–',
        ncov !== null && ncov !== undefined ? pct(ncov) + ' on new code' : fmt(cov.covered_lines) + ' of ' + fmt(cov.coverable_lines) + ' lines'));
    }
    add(main, add(panel('Measures'), cards, capNote()));

    // Findings by severity
    var s = panel('Findings by severity');
    var max = Math.max.apply(null, D.counts.total.concat([1]));
    var head = hasBase ? ['Severity', 'New', 'Worsened', 'Fixed', 'Total', ''] : ['Severity', 'Total', ''];
    var num = hasBase ? [0, 1, 1, 1, 1, 0] : [0, 1, 0];
    var t2 = table(head, num);
    for (var i = 0; i < 3; i++) {
      var b = el('td', 'barcell');
      var cells = hasBase
        ? [sevBadge(i), fmt(D.counts.new[i]), fmt(D.counts.worsened[i]), fmt(D.counts.fixed[i]), fmt(D.counts.total[i])]
        : [sevBadge(i), fmt(D.counts.total[i])];
      var tr = row(t2.body, cells, num);
      add(tr, add(b, bar(D.counts.total[i], max)));
    }
    add(s, t2.wrap);
    if (D.counts.incubating) {
      add(s, el('p', 'note', fmt(D.counts.incubating) + ' of these come from incubating rules: they report, but never gate and do not count toward effort or grades.'));
    }
    add(main, s);

    // By quality
    var q = panel('Findings by quality');
    var t3 = table(['Quality', 'HIGH', 'MEDIUM', 'LOW', 'Estimated effort'], [0, 1, 1, 1, 1]);
    [['Reliability', D.counts.reliability, m.effort_minutes.reliability],
      ['Maintainability', D.counts.maintainability, m.effort_minutes.maintainability]].forEach(function (x) {
      row(t3.body, [x[0], fmt(x[1][0]), fmt(x[1][1]), fmt(x[1][2]), hours(x[2])], [0, 1, 1, 1, 1]);
    });
    add(q, t3.wrap);
    add(main, q);

    // By rule
    var rs = D.rules.filter(function (r) { return r.count > 0; });
    rs.sort(function (a, b) {
      return (hasBase ? b.new - a.new : 0) || b.count - a.count || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
    });
    if (rs.length) {
      var p = panel('Findings by rule' + (rs.length > 20 ? ' (top 20 of ' + fmt(rs.length) + ')' : ''));
      var h4 = hasBase ? ['Rule', 'Severity', 'New or worsened', 'Total', ''] : ['Rule', 'Severity', 'Total', ''];
      var n4 = hasBase ? [0, 0, 1, 1, 0] : [0, 0, 1, 0];
      var t4 = table(h4, n4);
      var rmax = rs.length ? rs[0].count : 1;
      rs.forEach(function (r) { rmax = Math.max(rmax, r.count); });
      rs.slice(0, 20).forEach(function (r) {
        var cells = [link('#rule=' + encodeURIComponent(r.id), r.id), sevBadge(SEV.indexOf(r.severity))];
        if (hasBase) cells.push(fmt(r.new));
        cells.push(fmt(r.count));
        var tr = row(t4.body, cells, n4);
        add(tr, add(el('td', 'barcell'), bar(r.count, rmax)));
      });
      add(p, t4.wrap, link('#rules', 'All rules'));
      add(main, p);
    }

    // Main vs test, languages
    var mt = panel('Code');
    var t5 = table(['', 'Files', 'Lines of code', 'Findings', 'Cognitive complexity', 'Estimated effort', 'Duplication'], [0, 1, 1, 1, 1, 1, 1]);
    function mrow(label, x) {
      row(t5.body, [label, fmt(x.files), fmt(x.ncloc), fmt(x.findings), fmt(x.cognitive_complexity),
        hours(x.effort_minutes.total), pct(x.duplicated_lines_density)], [0, 1, 1, 1, 1, 1, 1]);
    }
    mrow('Main code', D.measures.main);
    mrow('Test code', D.measures.test);
    Object.keys(D.measures.languages).forEach(function (k) { mrow('Language: ' + k, D.measures.languages[k]); });
    mrow('Total', m);
    add(mt, t5.wrap);
    if (D.skipped_files) add(mt, el('p', 'note', fmt(D.skipped_files) + ' files skipped (excluded, generated, minified, too large or binary).'));
    add(main, mt);

    if (D.diagnostics_total) {
      var dg = panel('Diagnostics (' + fmt(D.diagnostics_total) + ')');
      var t6 = table(['Level', 'Path', 'Message']);
      D.diagnostics.forEach(function (d) { row(t6.body, [d.level, d.path || '', d.message]); });
      add(dg, t6.wrap);
      if (D.diagnostics_total > D.diagnostics.length) add(dg, el('p', 'note', 'The first ' + fmt(D.diagnostics.length) + ' are shown.'));
      add(main, dg);
    }
  }

  // ---------------------------------------------------------------- files
  function buildTree() {
    var root = { name: '', path: '', dirs: {}, files: [], c: [0, 0, 0], ncloc: 0, effort: 0, n: 0 };
    files.forEach(function (f) {
      var parts = f.path.split('/');
      var node = root;
      var chain = [root];
      for (var i = 0; i < parts.length - 1; i++) {
        var name = parts[i];
        if (!Object.prototype.hasOwnProperty.call(node.dirs, name)) {
          node.dirs[name] = { name: name, path: parts.slice(0, i + 1).join('/'), dirs: {}, files: [], c: [0, 0, 0], ncloc: 0, effort: 0, n: 0 };
        }
        node = node.dirs[name];
        chain.push(node);
      }
      node.files.push(f);
      chain.forEach(function (d) {
        for (var k = 0; k < 3; k++) d.c[k] += f.c[k];
        d.ncloc += f.ncloc;
        d.effort += f.effort;
        d.n += 1;
      });
    });
    return root;
  }
  var tree = null;

  function fileLine(f, showPath) {
    var r = el('div', 'row file');
    add(r, link('#file=' + encodeURIComponent(f.path), showPath ? f.path : f.path.split('/').pop(), 'mono'), counts(f.c));
    return r;
  }
  function dirNode(d) {
    var det = el('details');
    var head = el('summary');
    var r = el('span', 'row');
    add(r, el('span', 'dir mono', d.name + '/'), add(counts(d.c), ' · ' + fmt(d.ncloc) + ' lines of code'));
    add(head, r);
    add(det, head);
    var filled = false;
    det.addEventListener('toggle', function () {
      if (!det.open || filled) return;
      filled = true;
      fillDir(det, d);
    });
    return det;
  }
  function fillDir(container, d) {
    Object.keys(d.dirs).sort().forEach(function (k) {
      var sub = d.dirs[k];
      // Collapse single-child chains: a/b/c shown as one row.
      while (Object.keys(sub.dirs).length === 1 && sub.files.length === 0) {
        var only = sub.dirs[Object.keys(sub.dirs)[0]];
        only = Object.assign({}, only, { name: sub.name + '/' + only.name });
        sub = only;
      }
      add(container, dirNode(sub));
    });
    d.files.forEach(function (f) { add(container, fileLine(f, false)); });
  }
  function filesView() {
    if (!tree) tree = buildTree();
    var p = panel('Files (' + fmt(files.length) + ')');
    var f = el('div', 'filters');
    var q = el('input');
    q.type = 'search';
    q.placeholder = 'Filter by path';
    var only = el('input');
    only.type = 'checkbox';
    var onlyLabel = add(el('label'), only, ' only files with findings');
    add(f, q, onlyLabel);
    add(p, f);
    var list = el('div', 'tree');
    add(p, list);
    function render() {
      list.textContent = '';
      var text = q.value.trim().toLowerCase();
      if (!text && !only.checked) {
        fillDir(list, tree);
        return;
      }
      var hits = files.filter(function (x) {
        return (!text || x.path.toLowerCase().indexOf(text) >= 0) && (!only.checked || sum(x.c) > 0);
      });
      hits.slice(0, 500).forEach(function (x) { add(list, fileLine(x, true)); });
      if (hits.length > 500) add(list, el('p', 'note', fmt(hits.length - 500) + ' more files match; refine the filter.'));
      if (!hits.length) add(list, el('p', 'note', 'No file matches.'));
    }
    q.addEventListener('input', render);
    only.addEventListener('change', render);
    render();
    add(main, p);
  }
  function fileView(path) {
    var f = byPath[path];
    var p = panel(null);
    add(p, link('#files', '← Files'));
    if (!f) {
      add(p, el('p', null, 'No such file in this report.'));
      add(main, p);
      return;
    }
    add(p, el('h2', 'mono', f.path));
    var dl = el('dl', 'kv');
    function kv(k, v) { add(dl, el('dt', null, k), el('dd', null, v)); }
    if (f.lang) kv('Language', f.lang);
    kv('Test code', f.test ? 'yes' : 'no');
    kv('Lines of code', fmt(f.ncloc));
    kv('Estimated effort', hours(f.effort));
    if (f.change) kv('Change', f.change);
    add(p, dl);
    var list = (byFile[path] || []).slice().sort(function (a, b) { return a.line - b.line || a.col - b.col; });
    var total = sum(f.c);
    if (!total) {
      add(p, el('p', 'note', 'No findings.'));
    } else {
      var t = table(['Line', 'Severity', 'Rule', 'Message', ''], [1, 0, 0, 0, 0]);
      list.forEach(function (x) {
        var msg = el('td', 'msg', x.msg);
        var tr = row(t.body, [x.line + ':' + x.col, sevBadge(x.sev), link('#rule=' + encodeURIComponent(x.rule.id), x.rule.id)], [1, 0, 0]);
        add(tr, msg, add(el('td'), statusBadge(x.status)));
      });
      add(p, t.wrap);
      if (list.length < total) add(p, el('p', 'note warn', fmt(total - list.length) + ' less severe findings of this file are not listed (the page is capped).'));
    }
    add(main, p);
  }

  // ---------------------------------------------------------------- findings
  var fstate = { sev: [true, true, true], newOnly: false, rule: '', text: '', shown: PAGE };
  function findingsView() {
    var p = panel('Findings (' + fmt(D.total_findings) + ')');
    add(p, capNote());
    var f = el('div', 'filters');
    var boxes = SEV.map(function (s, i) {
      var b = el('input');
      b.type = 'checkbox';
      b.checked = fstate.sev[i];
      add(f, add(el('label'), b, ' ' + s));
      return b;
    });
    var nb = null;
    if (hasBase) {
      nb = el('input');
      nb.type = 'checkbox';
      nb.checked = fstate.newOnly;
      add(f, add(el('label'), nb, ' new or worsened only'));
    }
    var sel = el('select');
    add(sel, el('option', null, 'All rules'));
    sel.firstChild.value = '';
    D.rules.filter(function (r) { return r.count > 0; }).map(function (r) { return r.id; }).sort().forEach(function (id) {
      var o = el('option', null, id);
      o.value = id;
      add(sel, o);
    });
    sel.value = fstate.rule;
    var q = el('input');
    q.type = 'search';
    q.placeholder = 'Filter by path or message';
    q.value = fstate.text;
    add(f, sel, q);
    add(p, f);
    var out = el('div');
    add(p, out);
    function render() {
      fstate.sev = boxes.map(function (b) { return b.checked; });
      fstate.newOnly = nb ? nb.checked : false;
      fstate.rule = sel.value;
      fstate.text = q.value;
      var text = q.value.trim().toLowerCase();
      var hits = findings.filter(function (x) {
        return fstate.sev[x.sev] && (!fstate.newOnly || x.status === 1 || x.status === 3) &&
          (!fstate.rule || x.rule.id === fstate.rule) &&
          (!text || x.file.path.toLowerCase().indexOf(text) >= 0 || x.msg.toLowerCase().indexOf(text) >= 0);
      });
      out.textContent = '';
      add(out, el('p', 'note', fmt(hits.length) + ' findings match' + (hits.length > fstate.shown ? '; showing the first ' + fmt(fstate.shown) : '') + '.'));
      var t = table(['Severity', 'Location', 'Rule', 'Message', ''], [0, 0, 0, 0, 0]);
      hits.slice(0, fstate.shown).forEach(function (x) {
        var loc = link('#file=' + encodeURIComponent(x.file.path), x.file.path + ':' + x.line, 'mono');
        var tr = row(t.body, [sevBadge(x.sev), loc, link('#rule=' + encodeURIComponent(x.rule.id), x.rule.id)]);
        add(tr, el('td', 'msg', x.msg), add(el('td'), statusBadge(x.status)));
      });
      add(out, t.wrap);
      if (hits.length > fstate.shown) {
        var more = el('button', null, 'Show ' + fmt(Math.min(PAGE, hits.length - fstate.shown)) + ' more');
        more.addEventListener('click', function () { fstate.shown += PAGE; render(); });
        add(out, more);
      }
    }
    function reset() { fstate.shown = PAGE; render(); }
    boxes.forEach(function (b) { b.addEventListener('change', reset); });
    if (nb) nb.addEventListener('change', reset);
    sel.addEventListener('change', reset);
    q.addEventListener('input', reset);
    render();
    add(main, p);
  }

  // ---------------------------------------------------------------- rules
  function rulesView() {
    var p = panel('Rules (' + fmt(D.rules.length) + ' enabled)');
    var f = el('div', 'filters');
    var q = el('input');
    q.type = 'search';
    q.placeholder = 'Filter by id or name';
    var only = el('input');
    only.type = 'checkbox';
    add(f, q, add(el('label'), only, ' only rules with findings'));
    add(p, f);
    var out = el('div');
    add(p, out);
    function render() {
      var text = q.value.trim().toLowerCase();
      var t = table(['Rule', 'Name', 'Severity', 'Quality', 'Status', 'Findings'], [0, 0, 0, 0, 0, 1]);
      D.rules.filter(function (r) {
        return (!only.checked || r.count > 0) &&
          (!text || r.id.toLowerCase().indexOf(text) >= 0 || r.name.toLowerCase().indexOf(text) >= 0);
      }).forEach(function (r) {
        row(t.body, [link('#rule=' + encodeURIComponent(r.id), r.id, 'mono'), r.name, sevBadge(SEV.indexOf(r.severity)),
          r.quality, r.status, fmt(r.count)], [0, 0, 0, 0, 0, 1]);
      });
      out.textContent = '';
      add(out, t.wrap);
    }
    q.addEventListener('input', render);
    only.addEventListener('change', render);
    render();
    add(main, p);
  }
  function ruleView(id) {
    var r = rulesById[id];
    var p = panel(null);
    add(p, link('#rules', '← Rules'));
    if (!r) {
      add(p, el('p', null, 'No such rule in this report.'));
      add(main, p);
      return;
    }
    add(p, el('h2', 'mono', r.id));
    add(p, el('p', null, r.name));
    var dl = el('dl', 'kv');
    function kv(k, v) { add(dl, el('dt', null, k), add(el('dd'), v)); }
    kv('Severity', sevBadge(SEV.indexOf(r.severity)));
    kv('Quality', r.quality);
    kv('Status', r.status + (r.status === 'incubating' ? ' (reports, never gates, not counted in effort or grades)' : ''));
    kv('Language', r.language);
    kv('Remediation', r.remediation_minutes + ' min per finding (estimate)');
    if (r.tags.length) kv('Tags', r.tags.join(', '));
    kv('Findings', fmt(r.count) + (hasBase ? ' (' + fmt(r.new) + ' new or worsened)' : ''));
    add(p, dl);
    if (r.description) add(p, el('pre', 'desc', r.description));
    if (r.params.length) {
      var t = table(['Parameter', 'Default', 'Description']);
      r.params.forEach(function (x) { row(t.body, [el('code', null, x[0]), el('code', null, x[1]), x[2]]); });
      add(p, t.wrap);
    }
    var list = byRule[r.id] || [];
    if (list.length) {
      var t2 = table(['Severity', 'Location', 'Message', ''], [0, 0, 0, 0]);
      list.slice(0, 500).forEach(function (x) {
        var tr = row(t2.body, [sevBadge(x.sev), link('#file=' + encodeURIComponent(x.file.path), x.file.path + ':' + x.line, 'mono')]);
        add(tr, el('td', 'msg', x.msg), add(el('td'), statusBadge(x.status)));
      });
      add(p, t2.wrap);
      if (list.length > 500 || list.length < r.count) {
        add(p, el('p', 'note', 'Showing ' + fmt(Math.min(500, list.length)) + ' of ' + fmt(r.count) + ' findings; the Findings tab filters by rule.'));
      }
    }
    add(main, p);
  }

  // ---------------------------------------------------------------- routing
  function route() {
    var h = location.hash.slice(1);
    var tab = 'overview';
    var arg = null;
    if (h.indexOf('file=') === 0) { tab = 'files'; arg = ['file', decodeSafe(h.slice(5))]; }
    else if (h.indexOf('rule=') === 0) { tab = 'rules'; arg = ['rule', decodeSafe(h.slice(5))]; }
    else if (tabLinks[h]) tab = h;
    Object.keys(tabLinks).forEach(function (k) { tabLinks[k].className = k === tab ? 'on' : ''; });
    main.textContent = '';
    if (arg && arg[0] === 'file') fileView(arg[1]);
    else if (arg && arg[0] === 'rule') ruleView(arg[1]);
    else if (tab === 'files') filesView();
    else if (tab === 'findings') findingsView();
    else if (tab === 'rules') rulesView();
    else overview();
    window.scrollTo(0, 0);
  }
  function decodeSafe(s) {
    try { return decodeURIComponent(s); } catch (e) { return s; }
  }
  window.addEventListener('hashchange', route);
  route();
})();
