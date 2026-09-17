.pragma library

var MINIMUM_SCALE = 0.75
var MAXIMUM_SCALE = 2.0
var PERCENT_STEP = 5
var MINIMUM_PERCENT = Math.round(MINIMUM_SCALE * 100)
var MAXIMUM_PERCENT = Math.round(MAXIMUM_SCALE * 100)

function clamp(value) {
  var scale = Number(value)
  if (!isFinite(scale) || scale <= 0) return 1.0
  return Math.min(MAXIMUM_SCALE, Math.max(MINIMUM_SCALE, scale))
}

function scaleFromPercent(value) {
  var percent = Number(value)
  if (!isFinite(percent) || percent <= 0) return 1.0
  return clamp(Math.floor(percent) / 100)
}

function percentFromScale(value) {
  return Math.round(clamp(value) * 100)
}

function px(size, value, fallback) {
  var base = Number(size)
  if (!isFinite(base) || base <= 0) base = Number(fallback)
  if (!isFinite(base) || base <= 0) return 1
  return Math.max(1, Math.round(base * clamp(value)))
}
