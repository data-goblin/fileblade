.pragma library

var GLYPHS = {
  open: "\u{f03cc}",
  folder: "\u{f0770}",
  editor: "\u{f0cb6}",
  edit: "\u{f0cb6}",
  copy: "\u{f018f}",
  download: "\u{f0120}",
  rename: "\u{f0455}",
  description: "\u{f1a7d}",
  tags: "\u{f04fc}",
  refresh: "\u{f0450}",
  delete: "\u{f0a7a}",
  start: "\u{f040d}",
  stop: "\u{f0667}",
  reveal: "\u{f178b}",
  login: "\u{f0342}",
  more: "\u{f01d8}",
  action: "\u{f0142}"
}

var RULES = [
  [/^(open|launch|browse|portal|visit)/, "open"],
  [/^(copy|yank|clip)/, "copy"],
  [/^(download|export|save|fetch|pull)/, "download"],
  [/^rename/, "rename"],
  [/^(description|describe|comment|note)/, "description"],
  [/^tag/, "tags"],
  [/^(edit|modify|change|update|set)/, "edit"],
  [/^(refresh|reload|sync|rescan)/, "refresh"],
  [/^(delete|remove|trash|drop|destroy|purge)/, "delete"],
  [/^(start|run|play|resume|restart|deploy)/, "start"],
  [/^(stop|terminate|cancel|kill|halt|pause|abort)/, "stop"],
  [/^(reveal|show|locate)/, "reveal"],
  [/^(login|signin|sign-in|auth)/, "login"],
  [/^(more|menu|actions)/, "more"]
]

function role(id) {
  var key = String(id || "").toLowerCase()
  for (var i = 0; i < RULES.length; i++) if (RULES[i][0].test(key)) return RULES[i][1]
  return "action"
}

function glyph(id) {
  return GLYPHS[role(id)]
}

function named(name) {
  return GLYPHS[name] || GLYPHS.action
}

function urgent(id) {
  return role(id) === "delete"
}
