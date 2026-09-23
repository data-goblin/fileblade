.pragma library
.import "MediaDates.js" as Dates
.import "TreeOrder.js" as Order

function records(items, offsetMinutes) {
  return items.map(function(item, index) {
    return { index: index, item: item, date: Dates.normalize(item.date, item.datePrecision, offsetMinutes) }
  })
}

var MAX_FINE_BINS = 4096
function bin(key, level, start, end) {
  return { key: key, level: level, start: start, end: end, count: 0, indices: [] }
}

function include(target, record) { target.indices.push(record.index); target.count++ }

function build(records, level, scope, rule) {
  var membership = Object.create(null)
  if (scope) scope.indices.forEach(function(index) { membership[index] = true })
  var selected = scope ? records.filter(function(record) { return membership[record.index] === true }) : records
  var dated = selected.filter(function(record) { return record.date.year !== null })
  var bins = [], unknown = bin("unknown", "unknown", null, null), undated = bin("undated", "undated", null, null)
  var start = scope ? scope.start : null, end = scope ? scope.end : null
  if (!scope && dated.length) {
    var minimum = dated[0].date.year, maximumYear = minimum
    dated.forEach(function(record) { minimum = Math.min(minimum, record.date.year); maximumYear = Math.max(maximumYear, record.date.year) })
    start = Dates.ordinal(minimum, 1, 1)
    end = Dates.ordinal(maximumYear + 1, 1, 1)
  }
  if (start !== null && end !== null) {
    if (level === "months" && end - start > 366 * 10) level = "years"
    if (level === "weeks" || level === "days" || level === "hours") {
      var expected = level === "days" ? end - start
        : (level === "hours" ? (end - start) * 24 : Math.ceil((end - start) / 7))
      if (expected > MAX_FINE_BINS) throw new Error("Fine bins exceed the bin ceiling")
    }
    var cursor = start
    while (cursor < end) {
      var date = Dates.calendar(cursor), next, entry
      if (level === "years") {
        next = Math.min(end, Dates.ordinal(date.year + 1, 1, 1))
        entry = bin(String(date.year), "years", cursor, next)
        entry.year = date.year
      } else if (level === "months") {
        next = Math.min(end, Dates.ordinal(date.year, date.month + 1, 1))
        entry = bin(Dates.monthKey(date.year, date.month), "months", cursor, next)
        entry.year = date.year
        entry.month = date.month
      } else if (level === "weeks") {
        var week = Dates.week(cursor, rule)
        next = Math.min(end, week.start + 7)
        entry = bin(week.year + "-W" + Dates.pad(week.number), "weeks", cursor, next)
        entry.weekYear = week.year
        entry.week = week.number
      } else if (level === "days") {
        next = cursor + 1
        entry = bin(Dates.dayKey(cursor), "days", cursor, next)
      } else if (level === "hours") {
        var startOfDay = Math.floor(cursor)
        var hour = Math.round((cursor - startOfDay) * 24)
        next = Math.min(end, startOfDay + (hour + 1) / 24)
        entry = bin(Dates.dayKey(startOfDay) + "T" + Dates.pad(hour), "hours", cursor, next)
        entry.hour = hour
      } else throw new Error("Unknown media detail level")
      bins.push(entry)
      cursor = next
    }
  }
  selected.forEach(function(record) {
    var date = record.date
    if (date.year === null) { include(undated, record); return }
    if ((level !== "years" && date.month === null)
      || ((level === "weeks" || level === "days" || level === "hours") && date.day === null)
      || (level === "hours" && date.hour === null)) {
      include(unknown, record)
      return
    }
    var position = date.ordinal === null ? Dates.ordinal(date.year, date.month === null ? 1 : date.month, 1) : date.ordinal
    if (level === "hours") position += date.hour / 24
    var low = 0, high = bins.length - 1
    while (low <= high) {
      var middle = Math.floor((low + high) / 2), entry = bins[middle]
      if (position < entry.start) high = middle - 1
      else if (position >= entry.end) low = middle + 1
      else { include(entry, record); return }
    }
    include(unknown, record)
  })
  if (unknown.count) bins.push(unknown)
  if (!scope || undated.count) bins.push(undated)
  var maximum = bins.reduce(function(value, entry) { return Math.max(value, entry.count) }, 0)
  return { level: level, bins: bins, count: selected.length, maximum: maximum }
}

function ordered(items, sorts) {
  var ordering = Order.normalizeSorts(sorts)
  var source = records(items)
  source.forEach(function(record) {
    var row = record.item
    record.sortRow = Object.assign({}, row, { git_status: row.gitStatus, git_modified_count: row.gitModifiedCount,
      git_deleted_count: row.gitDeletedCount, git_new_count: row.gitNewCount,
      git_repo_name: row.gitRepoName, git_branch: row.gitBranch, git_worktree: row.gitWorktree })
  })
  return source.sort(function(a, b) {
    var tie = String(a.item.path).localeCompare(String(b.item.path), undefined, { numeric: true }) || a.index - b.index
    if (ordering.length) return Order.compareEntries(a.sortRow, b.sortRow, ordering) || tie
    var left = a.date, right = b.date
    if (left.year === null || right.year === null) return left.year === right.year ? a.index - b.index : (left.year === null ? 1 : -1)
    return left.year - right.year || (left.month === null ? 13 : left.month) - (right.month === null ? 13 : right.month)
      || (left.day === null ? 32 : left.day) - (right.day === null ? 32 : right.day)
      || (left.epoch === null || right.epoch === null ? 0 : left.epoch - right.epoch) || a.index - b.index
  }).map(function(record, index) { return { index: index, item: record.item, date: record.date } })
}

function compact(result, capacity) {
  if (result.bins.length <= capacity || result.level !== "years") return result
  var years = result.bins.filter(function(entry) { return entry.level === "years" })
  var extra = result.bins.filter(function(entry) { return entry.level !== "years" })
  var step = Math.ceil(years.length / Math.max(1, capacity - extra.length)), bins = []
  for (var i = 0; i < years.length; i += step) {
    var part = years.slice(i, i + step), first = part[0], last = part[part.length - 1]
    var entry = bin(first.key + "–" + last.key, "ranges", first.start, last.end)
    part.forEach(function(year) { entry.indices = entry.indices.concat(year.indices); entry.count += year.count })
    entry.indices.sort(function(a, b) { return a - b })
    bins.push(entry)
  }
  bins = bins.concat(extra)
  return { level: "ranges", bins: bins, count: result.count,
    maximum: bins.reduce(function(value, entry) { return Math.max(value, entry.count) }, 0) }
}

function withoutEmpty(result) {
  var bins = result.bins.filter(function(entry) { return entry.count > 0 })
  return { level: result.level, bins: bins, count: result.count, maximum: result.maximum }
}

function finer(records, chosen, capacity, rule, showEmpty) {
  var order = ["years", "months", "weeks", "days"]
  var from = order.indexOf(chosen.level)
  if (from < 0) return chosen
  var best = chosen
  for (var index = from + 1; index < order.length; index++) {
    var candidate = null
    try { candidate = build(records, order[index], null, rule) } catch (error) { break }
    if (showEmpty === false) candidate = withoutEmpty(candidate)
    if (!candidate || !candidate.bins.length) continue
    if (candidate.bins.length > capacity) break
    if (candidate.bins.length > best.bins.length) best = candidate
  }
  return best
}

function overview(records, capacity, rule, showEmpty) {
  var years = build(records, "years", null, rule)
  var yearOnly = records.some(function(record) { return record.date.year !== null && record.date.month === null })
  if (showEmpty === false) {
    if (!yearOnly) {
      var months = withoutEmpty(build(records, "months", null, rule))
      if (months.bins.length <= capacity) return finer(records, months, capacity, rule, showEmpty)
    }
    return compact(withoutEmpty(years), capacity)
  }
  var yearCount = years.bins.filter(function(entry) { return entry.level === "years" }).length
  return !yearOnly && yearCount * 12 + 1 <= capacity
    ? finer(records, build(records, "months", null, rule), capacity, rule, showEmpty)
    : compact(years, capacity)
}

function child(records, parent, capacity, rule, showEmpty) {
  var level = ({ ranges: "years", years: "months", months: "weeks", weeks: "days", days: "hours" })[parent.level]
  if (!level) return null
  var result = build(records, level, parent, rule)
  if (showEmpty === false) result = withoutEmpty(result)
  return compact(result, capacity)
}

function hourly(records, days, rule) {
  var maximum = 0
  var bins = days.bins.map(function(day) {
    if (day.level !== "days") return day
    var hours = build(day.indices.map(function(index) { return records[index] }), "hours", day, rule)
    var known = hours.bins.filter(function(entry) { return entry.level === "hours" })
    known.forEach(function(hour) { maximum = Math.max(maximum, hour.count) })
    return Object.assign({}, day, { hours: known })
  })
  return { level: "hours", bins: bins, count: days.count, maximum: maximum }
}

function geometry(bins, columns, pitch, tileHeight) {
  return bins.map(function(entry) {
    var segments = [], row = -2, length = 0
    entry.indices.forEach(function(index) {
      var next = Math.floor(index / columns)
      if (next === row) return
      if (next === row + 1) {
        var previous = segments[segments.length - 1]
        length += next * pitch + tileHeight - previous.bottom
        previous.bottom = next * pitch + tileHeight
      } else {
        segments.push({ top: next * pitch, bottom: next * pitch + tileHeight, offset: length })
        length += tileHeight
      }
      row = next
    })
    return { bin: entry, first: entry.indices.length ? entry.indices[0] : -1,
      last: entry.indices.length ? entry.indices[entry.indices.length - 1] : -1,
      top: segments.length ? segments[0].top : null, bottom: segments.length ? segments[segments.length - 1].bottom : null,
      segments: segments, length: length, columns: columns, pitch: pitch }
  })
}

function after(values, position, valueFor) {
  var low = 0, high = values.length
  while (low < high) {
    var middle = Math.floor((low + high) / 2)
    if (valueFor(values[middle]) <= position) low = middle + 1
    else high = middle
  }
  return low
}

function viewport(bounds, top, height) {
  var start = null, end = null, active = [], first = -1, firstIndex = Infinity
  bounds.forEach(function(bound, index) {
    var segments = bound.segments
    var low = after(segments, top, function(segment) { return segment.bottom })
    var intersects = low < segments.length && segments[low].top < top + height
    active.push(intersects)
    if (!intersects) return
    var rowStart = Math.max(0, Math.floor(top / bound.pitch)) * bound.columns
    var at = after(bound.bin.indices, rowStart - 1, function(value) { return value })
    if (at < bound.bin.indices.length && bound.bin.indices[at] < firstIndex) { firstIndex = bound.bin.indices[at]; first = index }
    for (var i = low; i < segments.length && segments[i].top < top + height; i++) {
      var segment = segments[i]
      var from = index + (segment.offset + Math.max(0, top - segment.top)) / Math.max(1, bound.length)
      var to = index + (segment.offset + Math.min(segment.bottom, top + height) - segment.top) / Math.max(1, bound.length)
      start = start === null ? from : Math.min(start, from)
      end = end === null ? to : Math.max(end, to)
    }
  })
  return { start: start, end: end, active: active, first: first }
}

function seek(bounds, index, fraction, contentHeight, viewportHeight) {
  if (index < 0 || index >= bounds.length || bounds[index].top === null) return null
  var bound = bounds[index]
  var offset = Math.max(0, Math.min(1, fraction)) * bound.length
  var at = after(bound.segments, offset, function(segment) { return segment.offset + segment.bottom - segment.top })
  var segment = bound.segments[Math.min(at, bound.segments.length - 1)]
  return Math.max(0, Math.min(Math.max(0, contentHeight - viewportHeight), segment.top + offset - segment.offset))
}
