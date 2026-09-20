const $ = (s) => document.querySelector(s);
const state = { trajectory: [], entries: [], dims: 8, snapshot: null, tenants: new Set() };

async function api(path, body) {
  const r = await fetch(path, body === undefined ? undefined : {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
  });
  const data = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(data.error || `HTTP ${r.status}`);
  return data;
}

function short(h) { return h ? h.slice(0, 12) + "…" + h.slice(-6) : "—"; }
function msg(el, text, ok) { el.textContent = text; el.className = "msg " + (ok ? "ok" : "err"); }

function drawChart() {
  const c = $("#chart"), ctx = c.getContext("2d");
  const W = c.width, H = c.height, pts = state.trajectory, max = Math.log2(state.dims);
  ctx.fillStyle = "#0A0A0A"; ctx.fillRect(0, 0, W, H);
  ctx.strokeStyle = "#2C2C2C"; ctx.lineWidth = 1;
  for (let i = 1; i < 4; i++) { const y = (H * i) / 4; ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke(); }
  ctx.fillStyle = "#6E6E6E"; ctx.font = "10px monospace";
  ctx.fillText(`H max ${max.toFixed(2)} bit`, 6, 12); ctx.fillText("0", 6, H - 4);
  if (pts.length < 2) return;
  ctx.strokeStyle = "#00FF41"; ctx.lineWidth = 1.5; ctx.beginPath();
  pts.forEach((p, i) => {
    const x = (i / (pts.length - 1)) * (W - 8) + 4, y = H - (p.entropy / max) * (H - 16) - 8;
    i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
  });
  ctx.stroke();
  const last = pts[pts.length - 1];
  ctx.fillStyle = "#00FF41";
  ctx.fillRect(W - 8, H - (last.entropy / max) * (H - 16) - 10, 4, 4);
}

function renderSnapshot(s) {
  if (!s) return;
  state.snapshot = s; state.dims = s.state.length;
  $("#tick").textContent = s.tick;
  $("#entropy").textContent = s.entropy.toFixed(4) + " bit";
  $("#digest").textContent = s.digest;
  const peak = Math.max(...s.state);
  $("#stateBars").innerHTML = s.state.map((v, i) =>
    `<div class="bar-item"><div class="track"><div class="fill" style="width:${(v / peak) * 100}%"></div></div><span>s${i} ${v.toFixed(4)}</span></div>`).join("");
}

function renderMatrix(m) {
  if (!m) return;
  $("#matrixTotals").textContent = `${m.total} total · ${m.verified} verified · ${m.divergent} divergent`;
  $("#pairs tbody").innerHTML = Object.entries(m.byPair).map(([k, v]) =>
    `<tr><td>${k}</td><td class="v">${v.verified}</td><td class="d">${v.divergent}</td></tr>`).join("") ||
    `<tr><td colspan="3" class="dim">no validations yet</td></tr>`;
}

function entryRow(e, isNew) {
  const payload = JSON.stringify(e.payload);
  return `<div class="entry${isNew ? " new" : ""}"><span class="dim">#${e.seq}</span><span class="t">${e.tenant}</span><span class="k">${e.kind}</span><span class="h" title="${e.hash}">${short(e.hash)}</span><span class="p" title="${payload.replace(/"/g, "&quot;")}">${payload}</span></div>`;
}

function renderLog() {
  const f = $("#filter").value;
  const rows = state.entries.filter((e) => !f || e.tenant === f).slice(-200).reverse();
  $("#log").innerHTML = rows.map((e) => entryRow(e)).join("");
}

function addEntry(e) {
  state.entries.push(e);
  if (state.entries.length > 1000) state.entries.shift();
  if (!state.tenants.has(e.tenant)) {
    state.tenants.add(e.tenant);
    $("#filter").insertAdjacentHTML("beforeend", `<option value="${e.tenant}">${e.tenant}</option>`);
  }
  const f = $("#filter").value;
  if (!f || e.tenant === f) $("#log").insertAdjacentHTML("afterbegin", entryRow(e, true));
  $("#wormLen").textContent = e.seq + 1;
  $("#head").textContent = short(e.hash); $("#head").title = e.hash;
}

function setLink(cls, text) { const el = $("#link"); el.className = "pill " + cls; el.textContent = text; }

function connect() {
  const es = new EventSource("/api/events");
  es.addEventListener("state", (ev) => {
    const d = JSON.parse(ev.data);
    renderSnapshot(d.tensor); state.trajectory = d.trajectory; drawChart(); renderMatrix(d.auditor);
    $("#wormLen").textContent = d.worm.length; $("#head").textContent = short(d.worm.headHash); $("#head").title = d.worm.headHash;
    setLink("ok", "LIVE");
    api("/api/worm?limit=200").then((entries) => {
      state.entries = []; state.tenants.clear(); $("#filter").length = 1;
      for (const e of entries) {
        state.entries.push(e);
        if (!state.tenants.has(e.tenant)) { state.tenants.add(e.tenant); $("#filter").insertAdjacentHTML("beforeend", `<option value="${e.tenant}">${e.tenant}</option>`); }
      }
      renderLog();
    });
  });
  es.addEventListener("pulse", (ev) => {
    const p = JSON.parse(ev.data);
    $("#tick").textContent = p.tick; $("#wormLen").textContent = p.wormLength;
    if (state.trajectory.length === 0 || state.trajectory[state.trajectory.length - 1].tick !== p.tick) {
      state.trajectory.push({ tick: p.tick, entropy: p.entropy });
      if (state.trajectory.length > 200) state.trajectory.shift();
      drawChart();
    }
  });
  es.addEventListener("worm", (ev) => {
    const e = JSON.parse(ev.data); addEntry(e);
    if (e.kind === "TENSOR_STEP") { renderSnapshot(e.payload); pushTraj(e.payload); }
    if (e.kind === "TENSOR_APPLY") { renderSnapshot(e.payload.snapshot); pushTraj(e.payload.snapshot); }
    if (e.kind === "CROSS_VALIDATION") api("/api/auditor/matrix").then(renderMatrix);
    if (e.kind === "SYSTEM" && e.payload.action === "reset") api("/api/state").then((d) => { renderSnapshot(d.tensor); state.trajectory = d.trajectory; drawChart(); });
  });
  es.onerror = () => { setLink("err", "LINK LOST"); };
  es.onopen = () => setLink("ok", "LIVE");
}

function pushTraj(s) {
  state.trajectory.push({ tick: s.tick, entropy: s.entropy });
  if (state.trajectory.length > 200) state.trajectory.shift();
  drawChart();
}

function identity() { return Array.from({ length: state.dims }, (_, i) => Array.from({ length: state.dims }, (_, j) => (i === j ? 1 : 0))); }
function shift() { return Array.from({ length: state.dims }, (_, i) => Array.from({ length: state.dims }, (_, j) => (j === (i + 1) % state.dims ? 1 : 0))); }
function setMatrix(m) { $("#matrix").value = m.map((r) => r.join(" ")).join("\n"); }
function readMatrix() {
  return $("#matrix").value.trim().split(/\n+/).map((line) => line.trim().split(/[\s,]+/).map(Number));
}

async function step(n) {
  const tenant = $("#tenant").value.trim();
  try { for (let i = 0; i < n; i++) await api("/api/tensor/step", { tenant }); msg($("#labMsg"), `step ×${n} committed to WORM`, true); }
  catch (e) { msg($("#labMsg"), e.message, false); }
}

$("#btnStep").onclick = () => step(1);
$("#btnStep10").onclick = () => step(10);
$("#btnIdentity").onclick = () => setMatrix(identity());
$("#btnShift").onclick = () => setMatrix(shift());
$("#btnApply").onclick = async () => {
  try { await api("/api/tensor/apply", { tenant: $("#tenant").value.trim(), matrix: readMatrix() }); msg($("#labMsg"), "matrix applied — TENSOR_APPLY committed", true); }
  catch (e) { msg($("#labMsg"), "REJECTED (fail-closed): " + e.message, false); }
};
$("#btnReset").onclick = async () => {
  try { await api("/api/tensor/reset", { seed: Number($("#seed").value) }); msg($("#labMsg"), "controller reset", true); }
  catch (e) { msg($("#labMsg"), e.message, false); }
};
$("#btnFill").onclick = () => { if (state.snapshot) { $("#dA").value = state.snapshot.digest; $("#dB").value = state.snapshot.digest; } };
$("#btnCross").onclick = async () => {
  try {
    const r = await api("/api/auditor/cross-validate", {
      tenantA: $("#tA").value.trim(), tenantB: $("#tB").value.trim(), subject: $("#subject").value.trim(),
      digestA: $("#dA").value.trim(), digestB: $("#dB").value.trim(),
    });
    msg($("#crossResult"), `${r.status} · matrixHash ${short(r.matrixHash)}`, r.status === "VERIFIED");
  } catch (e) { msg($("#crossResult"), "REJECTED: " + e.message, false); }
};
$("#btnVerify").onclick = async () => {
  const el = $("#verifyResult");
  try {
    const { verify } = await api("/api/worm/verify");
    el.className = "pill " + (verify.ok ? "ok" : "err");
    el.textContent = verify.ok ? `CHAIN OK · ${verify.length} entries` : `BROKEN @#${verify.brokenAt} · ${verify.reason}`;
  } catch (e) { el.className = "pill err"; el.textContent = e.message; }
};
$("#filter").onchange = renderLog;

setMatrix(identity());
connect();
