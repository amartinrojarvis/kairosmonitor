// Model.js — pure helpers for the Kairos bar widget/panel.
//
// Everything here is plain ES5 so it runs both inside the Quickshell QML
// engine (import "Model.js" as Model) and under node for quick tests.

var ICON_MEMORY = "\uDB80\uDF5B"; // nf-md-memory
var ICON_GPU = "\uDB81\uDE1A"; // nf-md-chip
var ICON_TPS = "\uDB85\uDC0B"; // nf-md-lightning_bolt

function num(value, fallback) {
  var n = Number(value)
  return isFinite(n) ? n : fallback
}

function str(value, fallback) {
  if (value === undefined || value === null) return fallback
  var s = String(value)
  return s === "" ? fallback : s
}

// parseMetrics — turn a Kairos Dash /api/metrics payload into the flat shape
// the widget and panel render. Never throws: a bad payload yields
// { ok: false, error }.
function parseMetrics(raw) {
  var out = {
    ok: false,
    error: "",
    hostname: "",
    uptime: -1,
    tailscale: "",
    memPercent: -1,
    memUsed: 0,
    memTotal: 0,
    swapPercent: -1,
    cpuPercent: -1,
    cpuCores: 0,
    load1: -1,
    load5: -1,
    load15: -1,
    gpuPercent: -1,
    gpuTemp: -1,
    gpuPower: -1,
    gpuName: "",
    tempMax: -1,
    netRx: 0,
    netTx: 0,
    diskPercent: -1,
    diskUsed: 0,
    diskTotal: 0,
    tps: -1,
    maxTps: -1,
    running: 0,
    waiting: 0,
    model: "",
    consumers: [],
    services: []
  }

  var text = String(raw || "").trim()
  if (text === "") {
    out.error = "empty response"
    return out
  }

  var data
  try {
    data = JSON.parse(text)
  } catch (e) {
    out.error = "invalid JSON"
    return out
  }
  if (!data || typeof data !== "object") {
    out.error = "unexpected payload"
    return out
  }

  out.hostname = str(data.hostname, "")
  out.uptime = num(data.uptime, -1)
  if (data.tailscale && Array.isArray(data.tailscale.ips) && data.tailscale.ips.length > 0) {
    out.tailscale = String(data.tailscale.ips[0])
  }

  var mem = data.memory || {}
  out.memPercent = num(mem.percent, -1)
  out.memUsed = num(mem.used, 0)
  out.memTotal = num(mem.total, 0)
  out.swapPercent = num(mem.swapPercent, -1)

  var cpu = data.cpu || {}
  out.cpuPercent = num(cpu.percent, -1)
  out.cpuCores = num(cpu.count, 0)
  if (Array.isArray(cpu.load) && cpu.load.length >= 3) {
    out.load1 = num(cpu.load[0], -1)
    out.load5 = num(cpu.load[1], -1)
    out.load15 = num(cpu.load[2], -1)
  }

  var gpus = Array.isArray(data.gpu) ? data.gpu : []
  if (gpus.length > 0) {
    var gpu = gpus[0] || {}
    out.gpuPercent = num(gpu.utilization, -1)
    out.gpuTemp = num(gpu.temperature, -1)
    out.gpuPower = num(gpu.powerDraw, -1)
    out.gpuName = str(gpu.name, "GPU")
  }
  if (data.temperature) out.tempMax = num(data.temperature.max, -1)

  var net = data.network || {}
  out.netRx = num(net.rxPerSec, 0)
  out.netTx = num(net.txPerSec, 0)

  if (Array.isArray(data.disk)) {
    for (var d = 0; d < data.disk.length; d++) {
      var part = data.disk[d]
      if (part && String(part.mount) === "/") {
        out.diskPercent = num(part.percent, -1)
        out.diskUsed = num(part.used, 0)
        out.diskTotal = num(part.total, 0)
        break
      }
    }
  }

  var tp = data.vllmThroughput || {}
  out.tps = num(tp.tps, -1)
  out.maxTps = num(tp.maxTps, -1)
  out.running = num(tp.running, 0)
  out.waiting = num(tp.waiting, 0)
  out.model = str(tp.model, "")
  if (Array.isArray(tp.consumers)) out.consumers = tp.consumers
  if (tp.ready === false || tp.ok === false) out.tps = -1

  // The dashboard also tracks the systemd/docker state of each vllm unit, so
  // a stopped critic reads as "--" instead of a stale number. Kept raw; the
  // UI picks its own service out of it with serviceFor().
  out.services = Array.isArray(data.services) ? data.services : []

  out.ok = out.memPercent >= 0 || out.gpuPercent >= 0 || out.tps >= 0
  if (!out.ok) out.error = "no metrics in payload"
  return out
}

// serviceFor — the dashboard's service record for a vllm unit id, or null.
function serviceFor(sample, label) {
  if (!sample || !Array.isArray(sample.services)) return null
  for (var i = 0; i < sample.services.length; i++) {
    var svc = sample.services[i]
    if (svc && String(svc.id) === String(label)) return svc
  }
  return null
}

// parseColors — flat key/value dict out of an Omarchy theme colors.toml.
// Only quoted string values are kept (that is where all the hex colors live).
function parseColors(raw) {
  var out = {}
  var lines = String(raw || "").split("\n")
  for (var i = 0; i < lines.length; i++) {
    var line = String(lines[i]).replace(/^\s+|\s+$/, "")
    if (!line || line.charAt(0) === "#") continue
    var kv = line.match(/^([A-Za-z0-9_-]+)\s*=\s*["']([^"']+)["']/)
    if (kv) out[kv[1].toLowerCase()] = kv[2]
  }
  return out
}

// themeColor — first hex found under any of the candidate keys, else fallback.
// Themes disagree on naming (tahoe-dark uses `green`, ANSI-style themes use
// `color2`/`color10`), so the caller passes the whole candidate list.
function themeColor(colors, keys, fallback) {
  if (!colors) return fallback
  for (var i = 0; i < keys.length; i++) {
    var v = colors[String(keys[i]).toLowerCase()]
    if (typeof v === "string" && /^#[0-9A-Fa-f]{3,9}$/.test(v)) return v
  }
  return fallback
}

function formatPercent(value) {
  var v = Number(value)
  if (!isFinite(v) || v < 0) return "--"
  return String(Math.round(v)) + "%"
}

function formatTps(value) {
  var v = Number(value)
  if (!isFinite(v) || v < 0) return "--"
  if (v >= 10) return String(Math.round(v))
  return String(Math.round(v * 10) / 10)
}

function formatBytes(bytes) {
  var units = ["B", "KiB", "MiB", "GiB", "TiB"]
  var v = Number(bytes) || 0
  var u = 0
  while (v >= 1024 && u < units.length - 1) {
    v = v / 1024
    u++
  }
  return (u === 0 || v >= 100 ? String(Math.round(v)) : String(Math.round(v * 10) / 10)) + " " + units[u]
}

// formatGiB — whole GiB with unit, the way the dashboard prints memory.
function formatGiB(bytes) {
  var v = Number(bytes) || 0
  return String(Math.round(v / 1073741824)) + " GiB"
}

// formatGiBShort — same number without the space, for the bar pill.
function formatGiBShort(bytes) {
  var v = Number(bytes) || 0
  var g = v / 1073741824
  return (g >= 10 ? String(Math.round(g)) : String(Math.round(g * 10) / 10)) + "G"
}

// formatRate — compact per-second byte rate, dashboard style (123K, 4.6G).
function formatRate(bytesPerSec) {
  var v = Number(bytesPerSec) || 0
  if (v >= 1073741824) return String(Math.round(v / 107374182.4) / 10) + "G"
  if (v >= 1048576) return String(Math.round(v / 104857.6) / 10) + "M"
  if (v >= 1024) return String(Math.round(v / 1024)) + "K"
  return String(Math.round(v))
}

function formatUptime(seconds) {
  var s = Math.max(0, Math.round(Number(seconds) || 0))
  var d = Math.floor(s / 86400)
  var h = Math.floor((s % 86400) / 3600)
  var m = Math.floor((s % 3600) / 60)
  var mm = m < 10 ? "0" + m : String(m)
  if (d > 0) return d + "d " + h + "h"
  if (h > 0) return h + "h " + mm + "m"
  return m + "m"
}

function formatClock(millis) {
  var date = new Date(Number(millis) || 0)
  function pad(n) { return n < 10 ? "0" + n : String(n) }
  return pad(date.getHours()) + ":" + pad(date.getMinutes()) + ":" + pad(date.getSeconds())
}

// stateOf — Kairos' three-tone state machine: ok / warn / danger.
function stateOf(value, warn, danger) {
  var v = Number(value)
  if (!isFinite(v) || v < 0) return "none"
  if (Number(danger) > 0 && v >= Number(danger)) return "danger"
  if (Number(warn) > 0 && v >= Number(warn)) return "warn"
  return "ok"
}

function memTooltip(sample) {
  if (!sample || sample.memPercent < 0) return "RAM: sin datos"
  var text = "RAM " + formatPercent(sample.memPercent)
  if (sample.memTotal > 0) {
    text += " · " + formatBytes(sample.memUsed) + " / " + formatBytes(sample.memTotal)
  }
  return text
}

function gpuTooltip(sample) {
  if (!sample || sample.gpuPercent < 0) return "GPU: sin datos"
  var text = str(sample.gpuName, "GPU") + " · " + formatPercent(sample.gpuPercent)
  if (sample.gpuTemp >= 0) text += " · " + Math.round(sample.gpuTemp) + "\u00B0C"
  if (sample.gpuPower >= 0) text += " · " + (Math.round(sample.gpuPower * 10) / 10) + " W"
  return text
}

function tpsTooltip(sample, label) {
  var name = str(label, "vllm")
  if (!sample || sample.tps < 0) return name + ": sin tr\u00E1fico"
  var text = name + " \u00B7 " + formatTps(sample.tps) + " tok/s"
  if (sample.maxTps >= 0) text += " \u00B7 m\u00E1x " + formatTps(sample.maxTps)
  text += " \u00B7 " + Math.round(sample.running) + " activa"
  if (sample.waiting > 0) text += " \u00B7 " + Math.round(sample.waiting) + " en cola"
  if (sample.model) text += " \u00B7 " + sample.model
  return text
}

// consumerLabels — "Hermes/domingo" style labels for the live vllm requests.
function consumerLabels(sample) {
  if (!sample || !Array.isArray(sample.consumers)) return []
  var labels = []
  for (var i = 0; i < sample.consumers.length; i++) {
    var c = sample.consumers[i]
    if (!c) continue
    var label = str(c.label, str(c.id, "consumidor"))
    var requests = num(c.requests, 0)
    labels.push(requests > 1 ? label + " x" + requests : label)
  }
  return labels
}

if (typeof module !== "undefined") {
  module.exports = {
    ICON_MEMORY: ICON_MEMORY,
    ICON_GPU: ICON_GPU,
    ICON_TPS: ICON_TPS,
    parseMetrics: parseMetrics,
    serviceFor: serviceFor,
    parseColors: parseColors,
    themeColor: themeColor,
    formatPercent: formatPercent,
    formatTps: formatTps,
    formatBytes: formatBytes,
    formatGiB: formatGiB,
    formatGiBShort: formatGiBShort,
    formatRate: formatRate,
    formatUptime: formatUptime,
    formatClock: formatClock,
    stateOf: stateOf,
    consumerLabels: consumerLabels,
    memTooltip: memTooltip,
    gpuTooltip: gpuTooltip,
    tpsTooltip: tpsTooltip
  }
}
