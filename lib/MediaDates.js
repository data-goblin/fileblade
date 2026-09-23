.pragma library

var DAY = 86400000
var FOUR_DAY_TERRITORIES = "AD AN AT AX BE BG CH CZ DE DK EE ES FI FJ FO FR GB GF GG GI GP GR HU IE IM IS IT JE LI LT LU MC MQ NL NO PL PT RE RU SE SJ SK SM VA".split(" ")

function localeRule(name, firstDay) {
  var parts = String(name || "").replace(/-/g, "_").split("_")
  var territory = "001"
  for (var i = 1; i < parts.length; i++) {
    if (/^[A-Z]{2}$|^\d{3}$/.test(parts[i])) { territory = parts[i]; break }
  }
  return { firstDay: Number(firstDay) === 0 ? 7 : Math.max(1, Math.min(7, Number(firstDay) || 1)),
    minimumDays: FOUR_DAY_TERRITORIES.indexOf(territory) >= 0 ? 4 : 1, territory: territory }
}

function ordinal(year, month, day) {
  var date = new Date(0)
  date.setUTCFullYear(year, month - 1, day)
  date.setUTCHours(0, 0, 0, 0)
  return Math.floor(date.getTime() / DAY)
}

function calendar(day) {
  var date = new Date(day * DAY)
  return { year: date.getUTCFullYear(), month: date.getUTCMonth() + 1, day: date.getUTCDate() }
}

function valid(year, month, day) {
  if (year < 1 || year > 9999 || month < 1 || month > 12 || day < 1 || day > 31) return false
  var actual = calendar(ordinal(year, month, day))
  return actual.year === year && actual.month === month && actual.day === day
}

function undated() { return { year: null, month: null, day: null, ordinal: null, epoch: null, hour: null, precision: "unknown" } }

function normalize(value, precision, offsetMinutes) {
  if (value && typeof value === "object" && !(value instanceof Date)) {
    precision = value.precision === undefined ? precision : value.precision
    value = value.value
  }
  if (value === undefined || value === null || value === "" || precision === "unknown") return undated()
  var year, month = null, day = null, epoch = null, available, match
  if (typeof value === "number" || value instanceof Date) {
    epoch = value instanceof Date ? value.getTime() : (Math.abs(value) < 1e11 ? value * 1000 : value)
    if (!isFinite(epoch)) return undated()
    available = "second"
  } else {
    match = /^(\d{4})(?:-(\d{2})(?:-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(\.\d{1,9})?)?(Z|[+-]\d{2}:?\d{2})?)?)?)?$/.exec(String(value))
    if (!match) return undated()
    year = Number(match[1])
    month = match[2] ? Number(match[2]) : null
    day = match[3] ? Number(match[3]) : null
    if (!valid(year, month === null ? 1 : month, day === null ? 1 : day)) return undated()
    available = day !== null ? "day" : (month !== null ? "month" : "year")
    if (match[4] !== undefined) {
      var hours = Number(match[4]), minutes = Number(match[5]), seconds = Number(match[6] || 0)
      if (hours > 23 || minutes > 59 || seconds > 59) return undated()
      available = match[7] ? "millisecond" : (match[6] ? "second" : "minute")
      var zone = match[8]
      if (zone) {
        var delta = 0
        if (zone !== "Z") {
          var digits = zone.slice(1).replace(":", "")
          if (Number(digits.slice(0, 2)) > 23 || Number(digits.slice(2)) > 59) return undated()
          delta = (Number(digits.slice(0, 2)) * 60 + Number(digits.slice(2))) * (zone[0] === "+" ? 1 : -1)
        }
        epoch = ordinal(year, month, day) * DAY + ((hours * 60 + minutes - delta) * 60 + seconds) * 1000 + Number(match[7] || 0) * 1000
      } else {
        var local = new Date(0)
        local.setFullYear(year, month - 1, day)
        local.setHours(hours, minutes, seconds, Number(match[7] || 0) * 1000)
        epoch = local.getTime()
      }
    }
  }
  if (epoch !== null) {
    var stamp = new Date(epoch + (offsetMinutes === undefined ? 0 : Number(offsetMinutes) * 60000))
    year = offsetMinutes === undefined ? stamp.getFullYear() : stamp.getUTCFullYear()
    month = (offsetMinutes === undefined ? stamp.getMonth() : stamp.getUTCMonth()) + 1
    day = offsetMinutes === undefined ? stamp.getDate() : stamp.getUTCDate()
    if (!valid(year, month, day)) return undated()
  }
  var levels = ["year", "month", "day", "minute", "second", "millisecond"]
  var requested = precision === undefined || precision === "" ? available : String(precision)
  if (levels.indexOf(requested) < 0) return undated()
  var effective = levels[Math.min(levels.indexOf(requested), levels.indexOf(available))]
  if (effective === "year") { month = null; day = null }
  else if (effective === "month") day = null
  return { year: year, month: month, day: day, precision: effective,
    ordinal: day === null ? null : ordinal(year, month, day), epoch: epoch,
    hour: epoch !== null && levels.indexOf(effective) >= 3 ? (offsetMinutes === undefined ? stamp.getHours() : stamp.getUTCHours()) : null }
}

function weekday(day) { return ((day + 3) % 7 + 7) % 7 + 1 }

function weekStart(day, rule) { return day - (weekday(day) - rule.firstDay + 7) % 7 }

function yearStart(year, rule) {
  var first = ordinal(year, 1, 1)
  var start = weekStart(first, rule)
  return first - start > 7 - rule.minimumDays ? start + 7 : start
}

function week(day, rule) {
  var year = calendar(day).year
  if (day < yearStart(year, rule)) year--
  else if (day >= yearStart(year + 1, rule)) year++
  return { year: year, number: Math.floor((day - yearStart(year, rule)) / 7) + 1, start: weekStart(day, rule) }
}

function pad(value) { return value < 10 ? "0" + value : String(value) }
function monthKey(year, month) { return String(year) + "-" + pad(month) }
function dayKey(day) { var date = calendar(day); return monthKey(date.year, date.month) + "-" + pad(date.day) }
