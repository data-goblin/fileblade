.pragma library

var PLACEMENTS = ["below", "beside"]
var DEFAULT = "below"
var POSITIONS = ["top", "bottom", "left", "right"]

function normalize(value) {
  var wanted = String(value || "").toLowerCase()
  return PLACEMENTS.indexOf(wanted) >= 0 ? wanted : DEFAULT
}

function position(value) {
  var wanted = String(value || "").toLowerCase()
  return POSITIONS.indexOf(wanted) >= 0 ? wanted : "top"
}

function insets(edge, barPosition, barSize, barHidden, placement) {
  var size = barHidden || normalize(placement) !== "below" ? 0 : Math.max(0, Math.round(Number(barSize) || 0))
  var where = position(barPosition)
  var right = edge === "right"
  return {
    top: where === "top" ? size : 0,
    bottom: where === "bottom" ? size : 0,
    left: !right && where === "left" ? size : 0,
    right: right && where === "right" ? size : 0
  }
}

function reservesBeforeBar(placement) {
  return normalize(placement) === "beside"
}
